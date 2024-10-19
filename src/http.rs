use crate::api_traits::ApiOperation;
use crate::backoff::{Backoff, Exponential};
use crate::cache::{Cache, CacheState};
use crate::config::ConfigProperties;
use crate::error::GRError;
use crate::io::{
    parse_page_headers, parse_ratelimit_headers, FlowControlHeaders, HttpResponse, HttpRunner,
    RateLimitHeader, ResponseField,
};
use crate::time::{self, now_epoch_seconds, Milliseconds, Seconds};
use crate::{api_defaults, error, log_debug, log_error};
use crate::{log_info, Result};
use serde::{Deserialize, Serialize};
use std::borrow::Borrow;
use std::collections::{hash_map, HashMap};
use std::iter::Iterator;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use ureq::Error;

pub struct Client<C> {
    cache: C,
    config: Arc<dyn ConfigProperties>,
    refresh_cache: bool,
    time_to_ratelimit_reset: Mutex<Seconds>,
    remaining_requests: Mutex<u32>,
}

// TODO: provide builder pattern for Client.
impl<C> Client<C> {
    pub fn new(cache: C, config: Arc<dyn ConfigProperties>, refresh_cache: bool) -> Self {
        let remaining_requests = Mutex::new(api_defaults::DEFAULT_NUMBER_REQUESTS_MINUTE);
        let time_to_ratelimit_reset = Mutex::new(now_epoch_seconds() + Seconds::new(60));
        Client {
            cache,
            refresh_cache,
            config,
            time_to_ratelimit_reset,
            remaining_requests,
        }
    }

    fn submit<T: Serialize>(&self, request: &Request<T>) -> Result<HttpResponse> {
        let ureq_req = match request.method {
            Method::GET => ureq::get(request.url()),
            Method::HEAD => ureq::head(request.url()),
            Method::POST => ureq::post(request.url()),
            Method::PATCH => ureq::patch(request.url()),
            Method::PUT => ureq::put(request.url()),
        };
        let ureq_req = request
            .headers()
            .iter()
            .fold(ureq_req, |req, (key, value)| req.set(key, value));
        let call = || -> std::result::Result<ureq::Response, ureq::Error> {
            match request.method {
                Method::GET | Method::HEAD => ureq_req.call(),
                _ => ureq_req.send_json(serde_json::to_value(request.body).unwrap()),
            }
        };
        match call() {
            Ok(response) | Err(Error::Status(_, response)) => {
                let status = response.status().into();
                // Grab headers for pagination and cache.
                let headers =
                    response
                        .headers_names()
                        .iter()
                        .fold(Headers::new(), |mut headers, name| {
                            headers.set(
                                name.to_lowercase(),
                                response.header(name.as_str()).unwrap().to_string(),
                            );
                            headers
                        });
                let rate_limit_header = Rc::new(parse_ratelimit_headers(Some(&headers)));
                let page_header = Rc::new(parse_page_headers(Some(&headers)));
                let flow_control_headers = FlowControlHeaders::new(page_header, rate_limit_header);
                // log debug response headers
                log_debug!("Response headers: {:?}", headers);
                let body = response.into_string().unwrap_or_default();
                let response = HttpResponse::builder()
                    .status(status)
                    .body(body)
                    .headers(headers)
                    .flow_control_headers(flow_control_headers)
                    .build()
                    .unwrap();
                self.handle_rate_limit(&response)?;
                Ok(response)
            }
            Err(err) => Err(GRError::HttpTransportError(err.to_string()).into()),
        }
    }
}

impl<C> Client<C> {
    fn handle_rate_limit(&self, response: &HttpResponse) -> Result<()> {
        if let Some(headers) = response.get_ratelimit_headers().borrow() {
            if headers.remaining <= self.config.rate_limit_remaining_threshold() {
                log_error!("Rate limit threshold reached");
                return Err(error::GRError::RateLimitExceeded(*headers).into());
            }
            Ok(())
        } else {
            // The remote does not provide rate limit headers, so we apply our
            // defaults for safety. Official github.com and gitlab.com do, so
            // that could be an internal/dev, etc... instance setup without rate
            // limits.
            log_info!("Rate limit headers not provided by remote, using defaults");
            default_rate_limit_handler(
                &self.config,
                &self.time_to_ratelimit_reset,
                &self.remaining_requests,
                now_epoch_seconds,
            )
        }
    }
}

fn default_rate_limit_handler(
    config: &Arc<dyn ConfigProperties>,
    time_to_ratelimit_reset: &Mutex<Seconds>,
    remaining_requests: &Mutex<u32>,
    now_epoch_seconds: fn() -> Seconds,
) -> Result<()> {
    // bail if we are below the security threshold for remaining requests

    if let Ok(remaining_requests) = remaining_requests.lock() {
        if *remaining_requests <= config.rate_limit_remaining_threshold() {
            let time_to_ratelimit_reset =
                *time_to_ratelimit_reset.lock().unwrap() - now_epoch_seconds();
            log_error!(
                "Remote does not provide rate limit headers, \
                            so the default rate limit of {} per minute has been \
                            exceeded. Try again in {} seconds",
                api_defaults::DEFAULT_NUMBER_REQUESTS_MINUTE,
                time_to_ratelimit_reset
            );
            return Err(error::GRError::RateLimitExceeded(RateLimitHeader::new(
                *remaining_requests,
                time_to_ratelimit_reset,
                time_to_ratelimit_reset,
            ))
            .into());
        }
    } else {
        return Err(error::GRError::ApplicationError(
            "http module rate limiting - Cannot read remaining \
                    http requests"
                .to_string(),
        )
        .into());
    }

    if let Ok(mut remaining_requests) = remaining_requests.lock() {
        // if elapsed time is greater than 60 seconds, then reset
        // remaining requests to default
        let current_time = now_epoch_seconds();
        let mut time_to_reset = time_to_ratelimit_reset.lock().unwrap();
        if current_time > *time_to_reset {
            *remaining_requests = api_defaults::DEFAULT_NUMBER_REQUESTS_MINUTE;
            *time_to_reset = current_time + Seconds::new(60);
        }
        *remaining_requests -= 1;
        // Using time to seconds relative as counter gets reset every minute.
        log_info!(
            "Remaining requests: {}, reset in: {} seconds",
            *remaining_requests,
            time::epoch_to_seconds_relative(*time_to_reset)
        );
    } else {
        return Err(error::GRError::ApplicationError(
            "http module rate limiting - Cannot decrease counter \
                        number of requests pending"
                .to_string(),
        )
        .into());
    }

    Ok(())
}

#[derive(Default)]
pub struct Resource {
    pub url: String,
    pub api_operation: Option<ApiOperation>,
}

impl Resource {
    pub fn new(url: &str, api_operation: Option<ApiOperation>) -> Self {
        Resource {
            url: url.to_string(),
            api_operation,
        }
    }
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct Body<T>(HashMap<String, T>);

impl<T> Body<T> {
    pub fn new() -> Self {
        Body(HashMap::new())
    }

    pub fn add<K: Into<String>>(&mut self, key: K, value: T) {
        self.0.insert(key.into(), value);
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Headers(HashMap<String, String>);

impl Headers {
    pub fn new() -> Self {
        Headers(HashMap::new())
    }

    pub fn set<K: Into<String>, V: Into<String>>(&mut self, key: K, value: V) {
        self.0.insert(key.into(), value.into());
    }

    pub fn get(&self, key: &str) -> Option<&String> {
        self.0.get(key)
    }

    pub fn iter(&self) -> hash_map::Iter<String, String> {
        self.0.iter()
    }

    pub fn extend(&mut self, headers: Headers) {
        for (key, value) in headers.iter() {
            self.0.insert(key.clone(), value.clone());
        }
    }
}

#[derive(Builder)]
#[builder(pattern = "owned")]
pub struct Request<'a, T> {
    #[builder(setter(into, strip_option), default)]
    pub body: Option<&'a Body<T>>,
    #[builder(default)]
    headers: Headers,
    pub method: Method,
    pub resource: Resource,
    #[builder(setter(into, strip_option), default)]
    pub max_pages: Option<i64>,
}

impl<'a, T> Request<'a, T> {
    pub fn builder() -> RequestBuilder<'a, T> {
        RequestBuilder::default()
    }

    pub fn new(url: &str, method: Method) -> Self {
        Request {
            body: None,
            headers: Headers::new(),
            method,
            resource: Resource::new(url, None),
            max_pages: None,
        }
    }

    pub fn set_max_pages(&mut self, max_pages: i64) {
        self.max_pages = Some(max_pages);
    }

    pub fn with_api_operation(mut self, api_operation: ApiOperation) -> Self {
        self.resource.api_operation = Some(api_operation);
        self
    }

    pub fn api_operation(&self) -> &Option<ApiOperation> {
        &self.resource.api_operation
    }

    pub fn set_header(&mut self, key: &str, value: &str) {
        self.headers.set(key.to_string(), value.to_string());
    }

    pub fn set_headers(&mut self, headers: Headers) {
        self.headers = headers;
    }

    pub fn set_url(&mut self, url: &str) {
        self.resource.url = url.to_string();
    }

    pub fn url(&self) -> &str {
        &self.resource.url
    }

    pub fn headers(&self) -> &Headers {
        &self.headers
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum Method {
    #[default]
    HEAD,
    GET,
    POST,
    PUT,
    PATCH,
}

impl<C: Cache<Resource>> HttpRunner for Client<C> {
    type Response = HttpResponse;

    fn run<T: Serialize>(&self, cmd: &mut Request<T>) -> Result<Self::Response> {
        match cmd.method {
            Method::GET => {
                let mut default_response = HttpResponse::builder().build().unwrap();
                match self.cache.get(&cmd.resource) {
                    Ok(CacheState::Fresh(response)) => {
                        log_debug!("Cache fresh for {}", cmd.resource.url);
                        if !self.refresh_cache {
                            log_debug!("Returning local cached response");
                            return Ok(response);
                        }
                        default_response = response;
                    }
                    Ok(CacheState::Stale(response)) => {
                        log_debug!("Cache stale for {}", cmd.resource.url);
                        default_response = response;
                    }
                    Ok(CacheState::None) => {}
                    Err(err) => return Err(err),
                }
                // check ETag is available in the default response.
                // If so, then we need to set the If-None-Match header.
                if let Some(etag) = default_response.get_etag() {
                    cmd.set_header("If-None-Match", etag);
                }
                // If status is 304, then we need to return the cached response.
                let response = self.submit(cmd)?;
                if response.status == 304 {
                    // Update cache with latest headers. This effectively
                    // refreshes the cache and we won't hit this until per api
                    // cache expiration as declared in the config.
                    self.cache
                        .update(&cmd.resource, &response, &ResponseField::Headers)?;
                    return Ok(default_response);
                }
                self.cache.set(&cmd.resource, &response).unwrap();
                Ok(response)
            }
            _ => Ok(self.submit(cmd)?),
        }
    }

    fn api_max_pages<T: Serialize>(&self, cmd: &Request<T>) -> u32 {
        let max_pages = self
            .config
            .get_max_pages(cmd.resource.api_operation.as_ref().unwrap());
        max_pages
    }
}

pub struct Paginator<'a, R, T> {
    runner: &'a Arc<R>,
    request: Request<'a, T>,
    page_url: Option<String>,
    iter: u32,
    throttle_time: Option<Milliseconds>,
    throttle_range: Option<(Milliseconds, Milliseconds)>,
    backoff: Backoff<'a, R>,
}

impl<'a, R, T> Paginator<'a, R, T> {
    pub fn new(
        runner: &'a Arc<R>,
        request: Request<'a, T>,
        page_url: &str,
        throttle_time: Option<Milliseconds>,
        throttle_range: Option<(Milliseconds, Milliseconds)>,
        backoff_max_retries: u32,
        backoff_default_wait_time: u64,
    ) -> Self {
        Paginator {
            runner,
            request,
            page_url: Some(page_url.to_string()),
            iter: 0,
            throttle_time,
            throttle_range,
            backoff: Backoff::new(
                runner,
                backoff_max_retries,
                backoff_default_wait_time,
                time::now_epoch_seconds,
                Box::new(Exponential),
            ),
        }
    }
}

impl<'a, T: Serialize, R: HttpRunner<Response = HttpResponse>> Iterator for Paginator<'a, R, T> {
    type Item = Result<HttpResponse>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(page_url) = &self.page_url {
            if let Some(max_pages) = self.request.max_pages {
                if self.iter >= max_pages as u32 {
                    return None;
                }
            } else if self.iter == self.runner.api_max_pages(&self.request) {
                return None;
            }
            if self.iter >= 1 {
                self.request.set_url(page_url);
                if let Some(throttle_time) = self.throttle_time {
                    log_info!("Throttling for: {} ms", throttle_time);
                    self.runner.throttle(throttle_time);
                } else if let Some((min, max)) = self.throttle_range {
                    log_info!("Throttling between: {} ms and {} ms", min, max);
                    self.runner.throttle_range(min, max);
                }
            }
            log_info!("Requesting page: {}", self.iter + 1);
            log_info!("URL: {}", self.request.url());

            let response = match self.backoff.retry_on_error(&mut self.request) {
                Ok(response) => {
                    if let Some(page_headers) = response.get_page_headers().borrow() {
                        match (page_headers.next_page(), page_headers.last_page()) {
                            (Some(next), _) => self.page_url = Some(next.url().to_string()),
                            (None, _) => self.page_url = None,
                        }
                    } else {
                        self.page_url = None;
                    };
                    Some(Ok(response))
                }
                Err(err) => {
                    self.page_url = None;
                    Some(Err(err))
                }
            };
            self.iter += 1;
            return response;
        }
        None
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use crate::{
        api_defaults::REST_API_MAX_PAGES,
        cache,
        io::{Page, PageHeader},
        test::utils::{init_test_logger, ConfigMock, MockRunner, LOG_BUFFER},
    };

    fn header_processor_next_page_no_last() -> Rc<Option<PageHeader>> {
        let mut page_header = PageHeader::new();
        page_header.set_next_page(Page::new("http://localhost?page=2", 1));
        Rc::new(Some(page_header))
    }

    fn header_processor_last_page_no_next() -> Rc<Option<PageHeader>> {
        let mut page_header = PageHeader::new();
        page_header.set_last_page(Page::new("http://localhost?page=2", 1));
        Rc::new(Some(page_header))
    }

    fn response_with_next_page() -> HttpResponse {
        let mut headers = Headers::new();
        headers.set("link".to_string(), "http://localhost?page=2".to_string());
        let flow_control_headers =
            FlowControlHeaders::new(header_processor_next_page_no_last(), Rc::new(None));
        let response = HttpResponse::builder()
            .status(200)
            .headers(headers)
            .flow_control_headers(flow_control_headers)
            .build()
            .unwrap();
        response
    }

    fn response_with_last_page() -> HttpResponse {
        let mut headers = Headers::new();
        headers.set("link".to_string(), "http://localhost?page=2".to_string());
        let flow_control_headers =
            FlowControlHeaders::new(header_processor_last_page_no_next(), Rc::new(None));
        let response = HttpResponse::builder()
            .status(200)
            .headers(headers)
            .flow_control_headers(flow_control_headers)
            .build()
            .unwrap();
        response
    }

    #[test]
    fn test_paginator_no_headers_no_next_no_last_pages() {
        let response = HttpResponse::builder().status(200).build().unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let request: Request<()> = Request::new("http://localhost", Method::GET);
        let paginator = Paginator::new(&client, request, "http://localhost", None, None, 0, 60);
        let responses = paginator.collect::<Vec<Result<HttpResponse>>>();
        assert_eq!(1, responses.len());
        assert_eq!("http://localhost", *client.url());
    }

    #[test]
    fn test_paginator_with_link_headers_one_next_and_no_last_pages() {
        let response1 = response_with_next_page();
        let mut headers = Headers::new();
        headers.set("link".to_string(), "http://localhost?page=2".to_string());
        let response2 = HttpResponse::builder()
            .status(200)
            .headers(headers)
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response2, response1]));
        let request: Request<()> = Request::new("http://localhost", Method::GET);
        let paginator = Paginator::new(&client, request, "http://localhost", None, None, 0, 60);
        let responses = paginator.collect::<Vec<Result<HttpResponse>>>();
        assert_eq!(2, responses.len());
    }

    #[test]
    fn test_paginator_with_link_headers_one_next_and_one_last_pages() {
        let response1 = response_with_next_page();
        let response2 = response_with_last_page();
        let client = Arc::new(MockRunner::new(vec![response2, response1]));
        let request: Request<()> = Request::new("http://localhost", Method::GET);
        let paginator = Paginator::new(&client, request, "http://localhost", None, None, 0, 60);
        let responses = paginator.collect::<Vec<Result<HttpResponse>>>();
        assert_eq!(2, responses.len());
    }

    #[test]
    fn test_paginator_error_response() {
        let response = HttpResponse::builder()
            .status(500)
            .body("Internal Server Error".to_string())
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let request: Request<()> = Request::new("http://localhost", Method::GET);
        let paginator = Paginator::new(&client, request, "http://localhost", None, None, 0, 60);
        let responses = paginator.collect::<Vec<Result<HttpResponse>>>();
        assert_eq!(1, responses.len());
        assert_eq!("http://localhost", *client.url());
    }

    #[test]
    fn test_client_get_api_max_pages() {
        let config = Arc::new(ConfigMock::new(1));
        let runner = Client::new(cache::NoCache, config, false);
        let cmd: Request<()> =
            Request::new("http://localhost", Method::GET).with_api_operation(ApiOperation::Project);
        assert_eq!(1, runner.api_max_pages(&cmd))
    }

    #[test]
    fn test_paginator_stops_paging_after_api_max_pages_reached() {
        let response1 = response_with_next_page();
        let response2 = response_with_next_page();
        let response3 = response_with_last_page();
        // setup client with max pages set to 1
        let client = Arc::new(
            MockRunner::new(vec![response3, response2, response1]).with_config(ConfigMock::new(1)),
        );
        let request: Request<()> = Request::new("http://localhost", Method::GET);
        let paginator = Paginator::new(&client, request, "http://localhost", None, None, 0, 60);
        let responses = paginator.collect::<Vec<Result<HttpResponse>>>();
        assert_eq!(1, responses.len());
    }

    #[test]
    fn test_paginator_limits_to_max_pages_default() {
        let api_max_pages = REST_API_MAX_PAGES + 5;
        let mut responses = Vec::new();
        for _ in 0..api_max_pages {
            let response = response_with_next_page();
            responses.push(response);
        }
        let last_response = response_with_last_page();
        responses.push(last_response);
        responses.reverse();
        let request: Request<()> = Request::new("http://localhost", Method::GET);
        let client = Arc::new(MockRunner::new(responses));
        let paginator = Paginator::new(&client, request, "http://localhost", None, None, 0, 60);
        let responses = paginator.collect::<Vec<Result<HttpResponse>>>();
        assert_eq!(REST_API_MAX_PAGES, responses.len() as u32);
    }

    #[test]
    fn test_ratelimit_remaining_threshold_reached_is_error() {
        let mut headers = Headers::new();
        headers.set("x-ratelimit-remaining".to_string(), "10".to_string());
        let flow_control_headers = FlowControlHeaders::new(
            Rc::new(None),
            Rc::new(Some(RateLimitHeader::new(
                10,
                Seconds::new(60),
                Seconds::new(60),
            ))),
        );
        let response = HttpResponse::builder()
            .status(200)
            .headers(headers)
            .flow_control_headers(flow_control_headers)
            .build()
            .unwrap();
        let client = Client::new(cache::NoCache, Arc::new(ConfigMock::new(1)), false);
        assert!(client.handle_rate_limit(&response).is_err());
    }

    #[test]
    fn test_ratelimit_remaining_threshold_not_reached_is_ok() {
        let mut headers = Headers::new();
        headers.set("ratelimit-remaining".to_string(), "11".to_string());
        let response = HttpResponse::builder()
            .status(200)
            .headers(headers)
            .build()
            .unwrap();
        let client = Client::new(cache::NoCache, Arc::new(ConfigMock::new(1)), false);
        assert!(client.handle_rate_limit(&response).is_ok());
    }

    fn epoch_seconds_now_mock(secs: u64) -> Seconds {
        Seconds::new(secs)
    }

    #[test]
    fn test_remaining_requests_below_threshold_all_fail() {
        // remaining requests - below threshold of 10 (api_defaults)
        let remaining_requests = Arc::new(Mutex::new(4));
        // 10 seconds before we reset counter to 80 (api_defaults)
        let time_to_ratelimit_reset = Arc::new(Mutex::new(Seconds::new(10)));
        let now = || -> Seconds { epoch_seconds_now_mock(1) };
        let config: Arc<dyn ConfigProperties> = Arc::new(ConfigMock::new(1));

        // counter will never get reset - all requests will fail
        let mut threads = Vec::new();
        for _ in 0..10 {
            let remaining_requests = remaining_requests.clone();
            let time_to_ratelimit_reset = time_to_ratelimit_reset.clone();
            let config = config.clone();
            threads.push(std::thread::spawn(move || {
                let result = default_rate_limit_handler(
                    &config,
                    &time_to_ratelimit_reset,
                    &remaining_requests,
                    now,
                );
                assert!(result.is_err());
            }));
        }
        for thread in threads {
            thread.join().unwrap();
        }
    }

    #[test]
    fn test_time_to_reset_achieved_resets_counter_all_ok() {
        // one remaining request before we hit threshold
        let remaining_requests = Arc::new(Mutex::new(11));
        let time_to_ratelimit_reset = Arc::new(Mutex::new(Seconds::new(1)));
        // now > time_to_ratelimit_reset - counter will be reset to 80
        let now = || -> Seconds { epoch_seconds_now_mock(2) };
        let config: Arc<dyn ConfigProperties> = Arc::new(ConfigMock::new(1));

        let mut threads = Vec::new();
        // 70 parallel requests - remaining 11.
        // On first request, will reset total number to 81, then decrease by 1.\
        // having 80 left to process with a threshold of 10, then the remaining
        // 69 will be processed. Time to reset will be set to 62
        // If we had 71, then the last one would fail.
        for _ in 0..70 {
            let remaining_requests = remaining_requests.clone();
            let time_to_ratelimit_reset = time_to_ratelimit_reset.clone();
            let config = config.clone();
            threads.push(std::thread::spawn(move || {
                let result = default_rate_limit_handler(
                    &config,
                    &time_to_ratelimit_reset,
                    &remaining_requests,
                    now,
                );
                assert!(result.is_ok());
            }));
        }
        for thread in threads {
            thread.join().unwrap();
        }
    }

    #[test]
    fn test_paginator_stops_paging_after_http_request_max_pages_reached() {
        let response1 = response_with_next_page();
        let response2 = response_with_next_page();
        let response3 = response_with_last_page();
        let client = Arc::new(MockRunner::new(vec![response3, response2, response1]));
        let request: Request<()> = Request::builder()
            .method(Method::GET)
            .resource(Resource::new("http://localhost", None))
            .max_pages(1)
            .build()
            .unwrap();
        let paginator = Paginator::new(&client, request, "http://localhost", None, None, 0, 60);
        let responses = paginator.collect::<Vec<Result<HttpResponse>>>();
        assert_eq!(1, responses.len());
    }

    #[test]
    fn test_paginator_fixed_throttle_enabled() {
        init_test_logger();
        let response1 = response_with_next_page();
        let response2 = response_with_next_page();
        let response3 = response_with_last_page();
        let client = Arc::new(MockRunner::new(vec![response3, response2, response1]));
        let request: Request<()> = Request::new("http://localhost", Method::GET);
        let paginator = Paginator::new(
            &client,
            request,
            "http://localhost",
            Some(Milliseconds::new(1)),
            None,
            0,
            60,
        );
        let responses = paginator.collect::<Vec<Result<HttpResponse>>>();
        assert_eq!(3, responses.len());
        let buffer = LOG_BUFFER.lock().unwrap();
        assert!(buffer.contains("Throttling for: 1 ms"));
        assert_eq!(Milliseconds::new(2), *client.milliseconds_throttled());
    }

    #[test]
    fn test_paginator_range_throttle_enabled() {
        init_test_logger();
        let response1 = response_with_next_page();
        let response2 = response_with_last_page();
        let client = Arc::new(MockRunner::new(vec![response2, response1]));
        let request: Request<()> = Request::new("http://localhost", Method::GET);
        let paginator = Paginator::new(
            &client,
            request,
            "http://localhost",
            None,
            Some((Milliseconds::new(1), Milliseconds::new(3))),
            0,
            60,
        );
        let responses = paginator.collect::<Vec<Result<HttpResponse>>>();
        assert_eq!(2, responses.len());
        let buffer = LOG_BUFFER.lock().unwrap();
        assert!(buffer.contains("Throttling between: 1 ms and 3 ms"));
        assert_eq!(1, *client.throttled());
    }

    #[test]
    fn test_user_request_from_up_to_pages_takes_over_max_api_pages() {
        let mut responses = Vec::new();
        for _ in 0..4 {
            let response = response_with_next_page();
            responses.push(response);
        }
        let last_response = response_with_last_page();
        responses.push(last_response);
        responses.reverse();
        // config api max pages 2
        let client = Arc::new(MockRunner::new(responses).with_config(ConfigMock::new(2)));
        let request: Request<()> = Request::builder()
            .method(Method::GET)
            .resource(Resource::new("http://localhost", None))
            // User requests 5 pages
            .max_pages(5)
            .build()
            .unwrap();
        let paginator = Paginator::new(&client, request, "http://localhost", None, None, 0, 60);
        let responses = paginator.collect::<Vec<Result<HttpResponse>>>();
        assert_eq!(5, responses.len());
    }
}
