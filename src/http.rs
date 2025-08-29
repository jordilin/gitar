pub mod throttle;

use crate::api_traits::ApiOperation;
use crate::backoff::Backoff;
use crate::cache::{Cache, CacheState};
use crate::config::ConfigProperties;
use crate::error::GRError;
use crate::io::{
    parse_page_headers, parse_ratelimit_headers, FlowControlHeaders, HttpResponse, HttpRunner,
    RateLimitHeader, ResponseField,
};
use crate::time::{self, now_epoch_seconds, Seconds};
use crate::{api_defaults, error, log_debug, log_error};
use crate::{log_info, Result};
use serde::{Deserialize, Serialize};
use std::borrow::Borrow;
use std::collections::{hash_map, HashMap};
use std::iter::Iterator;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use throttle::{ThrottleStrategy, ThrottleStrategyType};

pub struct Client<C> {
    cache: C,
    config: Arc<dyn ConfigProperties>,
    refresh_cache: bool,
    time_to_ratelimit_reset: Mutex<Seconds>,
    remaining_requests: Mutex<u32>,
    http_agent: ureq::Agent,
}

// TODO: provide builder pattern for Client.
impl<C> Client<C> {
    pub fn new(cache: C, config: Arc<dyn ConfigProperties>, refresh_cache: bool) -> Self {
        let remaining_requests = Mutex::new(api_defaults::DEFAULT_NUMBER_REQUESTS_MINUTE);
        let time_to_ratelimit_reset = Mutex::new(now_epoch_seconds() + Seconds::new(60));
        let http_config = ureq::Agent::config_builder()
            // Keeps same functionality as the ureq 2.x default.
            // that is we handle the response as normal when error codes such as
            // 4xx, 5xx are returned.
            .http_status_as_error(false)
            .build();
        Client {
            cache,
            refresh_cache,
            config,
            time_to_ratelimit_reset,
            remaining_requests,
            http_agent: http_config.into(),
        }
    }

    fn submit<T: Serialize>(&self, request: &Request<T>) -> Result<HttpResponse> {
        let response = match request.method {
            Method::GET => {
                let req = self.http_agent.get(request.url());
                let req = Self::add_headers(req, request.headers());
                req.call()
            }
            Method::HEAD => {
                let req = self.http_agent.head(request.url());
                let req = Self::add_headers(req, request.headers());
                req.call()
            }
            Method::POST => {
                let req = self.http_agent.post(request.url());
                let req = Self::add_headers(req, request.headers());
                req.send_json(serde_json::to_value(request.body).unwrap())
            }
            Method::PATCH => {
                let req = self.http_agent.patch(request.url());
                let req = Self::add_headers(req, request.headers());
                req.send_json(serde_json::to_value(request.body).unwrap())
            }
            Method::PUT => {
                let req = self.http_agent.put(request.url());
                let req = Self::add_headers(req, request.headers());
                req.send_json(serde_json::to_value(request.body).unwrap())
            }
        };

        match response {
            Ok(response) => {
                let status = response.status();
                // Grab headers for pagination and cache.
                let headers =
                    response
                        .headers()
                        .iter()
                        .fold(Headers::new(), |mut headers, (name, value)| {
                            headers.set::<String, String>(
                                name.to_string(),
                                value.to_str().unwrap().to_string(),
                            );
                            headers
                        });
                let rate_limit_header = Rc::new(parse_ratelimit_headers(Some(&headers)));
                let page_header = Rc::new(parse_page_headers(Some(&headers)));
                let flow_control_headers = FlowControlHeaders::new(page_header, rate_limit_header);
                // log debug response headers
                log_debug!("Response headers: {:?}", headers);
                let mut body = response.into_body();
                let mut response = HttpResponse::builder()
                    .status(status.as_u16() as i32)
                    .headers(headers)
                    .body(body.read_to_string().unwrap_or_default())
                    .flow_control_headers(flow_control_headers)
                    .build()
                    .unwrap();

                self.handle_rate_limit(&mut response)?;
                Ok(response)
            }
            Err(err) => Err(GRError::HttpTransportError(err.to_string()).into()),
        }
    }

    fn add_headers<B>(
        mut builder: ureq::RequestBuilder<B>,
        headers: &Headers,
    ) -> ureq::RequestBuilder<B> {
        for (key, value) in headers.iter() {
            builder = builder.header(key, value);
        }
        builder
    }
}

impl<C> Client<C> {
    fn handle_rate_limit(&self, response: &mut HttpResponse) -> Result<()> {
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
                response,
                &self.config,
                &self.time_to_ratelimit_reset,
                &self.remaining_requests,
                now_epoch_seconds,
            )
        }
    }
}

fn default_rate_limit_handler(
    response: &mut HttpResponse,
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
            let rate_limit_header = RateLimitHeader::new(
                *remaining_requests,
                time_to_ratelimit_reset,
                time_to_ratelimit_reset,
            );
            response.update_rate_limit_headers(rate_limit_header);
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

    pub fn iter(&self) -> hash_map::Iter<'_, String, String> {
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
                    Ok(CacheState::Fresh(mut response)) => {
                        log_debug!("Cache fresh for {}", cmd.resource.url);
                        if !self.refresh_cache {
                            log_debug!("Returning local cached response");
                            response.local_cache = true;
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
    request: Request<'a, T>,
    page_url: Option<String>,
    iter: u32,
    backoff: Backoff<'a, R>,
    throttler: Box<dyn ThrottleStrategy>,
    max_pages: u32,
}

impl<'a, R: HttpRunner, T: Serialize> Paginator<'a, R, T> {
    pub fn new(
        runner: &'a Arc<R>,
        request: Request<'a, T>,
        page_url: &str,
        backoff: Backoff<'a, R>,
        throttle_strategy: Box<dyn ThrottleStrategy>,
    ) -> Self {
        let max_pages = if let Some(max_pages) = request.max_pages {
            max_pages as u32
        } else {
            runner.api_max_pages(&request)
        };
        Paginator {
            request,
            page_url: Some(page_url.to_string()),
            iter: 0,
            backoff,
            throttler: throttle_strategy,
            max_pages,
        }
    }
}

impl<T: Serialize, R: HttpRunner<Response = HttpResponse>> Iterator for Paginator<'_, R, T> {
    type Item = Result<HttpResponse>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(page_url) = &self.page_url {
            if self.iter >= self.max_pages {
                return None;
            }
            if self.iter >= 1 {
                self.request.set_url(page_url);
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
                    Ok(response)
                }
                Err(err) => {
                    self.page_url = None;
                    Err(err)
                }
            };
            self.iter += 1;
            if self.iter < self.max_pages && self.page_url.is_some() {
                let response = response.as_ref().unwrap();
                // Technically no need to check ok on response, as page_url is Some
                // (response was Ok)
                if !response.local_cache {
                    if self.throttler.strategy() == ThrottleStrategyType::AutoRate {
                        if self.iter >= api_defaults::ENGAGE_AUTORATE_THROTTLING_THRESHOLD {
                            self.throttler
                                .throttle(Some(response.get_flow_control_headers()));
                        }
                    } else {
                        self.throttler
                            .throttle(Some(response.get_flow_control_headers()));
                    }
                }
            }
            return Some(response);
        }
        None
    }
}

#[cfg(test)]
mod test {
    use throttle::NoThrottle;

    use super::*;

    use crate::{
        api_defaults::REST_API_MAX_PAGES,
        backoff::Exponential,
        cache,
        io::{Page, PageHeader},
        test::utils::{ConfigMock, MockRunner, MockThrottler},
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

    fn cached_response_next_page() -> HttpResponse {
        let mut headers = Headers::new();
        headers.set("link".to_string(), "http://localhost?page=2".to_string());
        let flow_control_headers =
            FlowControlHeaders::new(header_processor_next_page_no_last(), Rc::new(None));
        let response = HttpResponse::builder()
            .status(200)
            .headers(headers)
            .flow_control_headers(flow_control_headers)
            .local_cache(true)
            .build()
            .unwrap();
        response
    }

    fn cached_response_last_page() -> HttpResponse {
        let mut headers = Headers::new();
        headers.set("link".to_string(), "http://localhost?page=2".to_string());
        let flow_control_headers =
            FlowControlHeaders::new(header_processor_last_page_no_next(), Rc::new(None));
        let response = HttpResponse::builder()
            .status(200)
            .headers(headers)
            .flow_control_headers(flow_control_headers)
            .local_cache(true)
            .build()
            .unwrap();
        response
    }

    #[test]
    fn test_paginator_no_headers_no_next_no_last_pages() {
        let response = HttpResponse::builder().status(200).build().unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let request: Request<()> = Request::new("http://localhost", Method::GET);
        let throttler: Box<dyn ThrottleStrategy> = Box::new(NoThrottle);
        let backoff = Backoff::new(
            &client,
            0,
            60,
            time::now_epoch_seconds,
            Box::new(Exponential),
            Box::new(throttle::DynamicFixed),
        );
        let paginator = Paginator::new(&client, request, "http://localhost", backoff, throttler);
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
        let throttler: Box<dyn ThrottleStrategy> = Box::new(NoThrottle);
        let backoff = Backoff::new(
            &client,
            0,
            60,
            time::now_epoch_seconds,
            Box::new(Exponential),
            Box::new(throttle::DynamicFixed),
        );
        let paginator = Paginator::new(&client, request, "http://localhost", backoff, throttler);
        let responses = paginator.collect::<Vec<Result<HttpResponse>>>();
        assert_eq!(2, responses.len());
    }

    #[test]
    fn test_paginator_with_link_headers_one_next_and_one_last_pages() {
        let response1 = response_with_next_page();
        let response2 = response_with_last_page();
        let client = Arc::new(MockRunner::new(vec![response2, response1]));
        let request: Request<()> = Request::new("http://localhost", Method::GET);
        let throttler: Box<dyn ThrottleStrategy> = Box::new(NoThrottle);
        let backoff = Backoff::new(
            &client,
            0,
            60,
            time::now_epoch_seconds,
            Box::new(Exponential),
            Box::new(throttle::DynamicFixed),
        );
        let paginator = Paginator::new(&client, request, "http://localhost", backoff, throttler);
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
        let throttler: Box<dyn ThrottleStrategy> = Box::new(NoThrottle);
        let backoff = Backoff::new(
            &client,
            0,
            60,
            time::now_epoch_seconds,
            Box::new(Exponential),
            Box::new(throttle::DynamicFixed),
        );
        let paginator = Paginator::new(&client, request, "http://localhost", backoff, throttler);
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
        let throttler: Box<dyn ThrottleStrategy> = Box::new(NoThrottle);
        let backoff = Backoff::new(
            &client,
            0,
            60,
            time::now_epoch_seconds,
            Box::new(Exponential),
            Box::new(throttle::DynamicFixed),
        );
        let paginator = Paginator::new(&client, request, "http://localhost", backoff, throttler);
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
        let throttler: Box<dyn ThrottleStrategy> = Box::new(NoThrottle);
        let backoff = Backoff::new(
            &client,
            0,
            60,
            time::now_epoch_seconds,
            Box::new(Exponential),
            Box::new(throttle::DynamicFixed),
        );
        let paginator = Paginator::new(&client, request, "http://localhost", backoff, throttler);
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
        let mut response = HttpResponse::builder()
            .status(200)
            .headers(headers)
            .flow_control_headers(flow_control_headers)
            .build()
            .unwrap();
        let client = Client::new(cache::NoCache, Arc::new(ConfigMock::new(1)), false);
        assert!(client.handle_rate_limit(&mut response).is_err());
    }

    #[test]
    fn test_ratelimit_remaining_threshold_not_reached_is_ok() {
        let mut headers = Headers::new();
        headers.set("ratelimit-remaining".to_string(), "11".to_string());
        let mut response = HttpResponse::builder()
            .status(200)
            .headers(headers)
            .build()
            .unwrap();
        let client = Client::new(cache::NoCache, Arc::new(ConfigMock::new(1)), false);
        assert!(client.handle_rate_limit(&mut response).is_ok());
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
                let mut response = HttpResponse::builder().status(200).build().unwrap();
                let result = default_rate_limit_handler(
                    &mut response,
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
                let mut response = HttpResponse::builder().status(200).build().unwrap();
                let result = default_rate_limit_handler(
                    &mut response,
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
        let throttler: Box<dyn ThrottleStrategy> = Box::new(NoThrottle);
        let backoff = Backoff::new(
            &client,
            0,
            60,
            time::now_epoch_seconds,
            Box::new(Exponential),
            Box::new(throttle::DynamicFixed),
        );
        let paginator = Paginator::new(&client, request, "http://localhost", backoff, throttler);
        let responses = paginator.collect::<Vec<Result<HttpResponse>>>();
        assert_eq!(1, responses.len());
    }

    #[test]
    fn test_paginator_fixed_throttle_enabled() {
        let response1 = response_with_next_page();
        let response2 = response_with_next_page();
        let response3 = response_with_last_page();
        let client = Arc::new(MockRunner::new(vec![response3, response2, response1]));
        let request: Request<()> = Request::new("http://localhost", Method::GET);
        let throttler = Rc::new(MockThrottler::new(None));
        let bthrottler: Box<dyn ThrottleStrategy> = Box::new(Rc::clone(&throttler));
        let backoff = Backoff::new(
            &client,
            0,
            60,
            time::now_epoch_seconds,
            Box::new(Exponential),
            Box::new(throttle::DynamicFixed),
        );
        let paginator = Paginator::new(&client, request, "http://localhost", backoff, bthrottler);
        let responses = paginator.collect::<Vec<Result<HttpResponse>>>();
        assert_eq!(3, responses.len());
        assert_eq!(2, *throttler.throttled());
    }

    #[test]
    fn test_paginator_range_throttle_enabled() {
        let response1 = response_with_next_page();
        let response2 = response_with_last_page();
        let client = Arc::new(MockRunner::new(vec![response2, response1]));
        let request: Request<()> = Request::new("http://localhost", Method::GET);
        let throttler = Rc::new(MockThrottler::new(None));
        let bthrottler: Box<dyn ThrottleStrategy> = Box::new(Rc::clone(&throttler));
        let backoff = Backoff::new(
            &client,
            0,
            60,
            time::now_epoch_seconds,
            Box::new(Exponential),
            Box::new(throttle::DynamicFixed),
        );
        let paginator = Paginator::new(&client, request, "http://localhost", backoff, bthrottler);
        let responses = paginator.collect::<Vec<Result<HttpResponse>>>();
        assert_eq!(2, responses.len());
        assert_eq!(1, *throttler.throttled());
    }

    #[test]
    fn test_paginator_no_throttle_if_response_is_from_local_cache() {
        let response1 = cached_response_next_page();
        let response2 = cached_response_next_page();
        let response3 = cached_response_last_page();
        let client = Arc::new(MockRunner::new(vec![response3, response2, response1]));
        let request: Request<()> = Request::new("http://localhost", Method::GET);
        let throttler = Rc::new(MockThrottler::new(None));
        let bthrottler: Box<dyn ThrottleStrategy> = Box::new(Rc::clone(&throttler));
        let backoff = Backoff::new(
            &client,
            0,
            60,
            time::now_epoch_seconds,
            Box::new(Exponential),
            Box::new(throttle::DynamicFixed),
        );
        let paginator = Paginator::new(&client, request, "http://localhost", backoff, bthrottler);
        let responses = paginator.collect::<Vec<Result<HttpResponse>>>();
        assert_eq!(3, responses.len());
        assert_eq!(0, *throttler.throttled());
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
        let backoff = Backoff::new(
            &client,
            0,
            60,
            time::now_epoch_seconds,
            Box::new(Exponential),
            Box::new(throttle::DynamicFixed),
        );
        let throttler: Box<dyn ThrottleStrategy> = Box::new(NoThrottle);
        let paginator = Paginator::new(&client, request, "http://localhost", backoff, throttler);
        let responses = paginator.collect::<Vec<Result<HttpResponse>>>();
        assert_eq!(5, responses.len());
    }

    #[test]
    fn test_paginator_auto_throttle_enabled_after_autorate_engage_threshold() {
        let response1 = response_with_next_page();
        let response2 = response_with_next_page();
        let response3 = response_with_next_page();
        // Throttles in next two requests
        let response4 = response_with_next_page();
        let response5 = response_with_last_page();
        let client = Arc::new(MockRunner::new(vec![
            response5, response4, response3, response2, response1,
        ]));
        let request: Request<()> = Request::builder()
            .method(Method::GET)
            .resource(Resource::new("http://localhost", None))
            .max_pages(5)
            .build()
            .unwrap();
        let throttler = Rc::new(MockThrottler::new(Some(
            throttle::ThrottleStrategyType::AutoRate,
        )));
        let bthrottler: Box<dyn ThrottleStrategy> = Box::new(Rc::clone(&throttler));
        let backoff = Backoff::new(
            &client,
            0,
            60,
            time::now_epoch_seconds,
            Box::new(Exponential),
            Box::new(throttle::DynamicFixed),
        );
        let paginator = Paginator::new(&client, request, "http://localhost", backoff, bthrottler);
        let responses = paginator.collect::<Vec<Result<HttpResponse>>>();
        assert_eq!(5, responses.len());
        assert_eq!(2, *throttler.throttled());
    }
}
