use crate::api_traits::ApiOperation;
use crate::cache::{Cache, CacheState};
use crate::config::ConfigProperties;
use crate::io::{HttpRunner, Response, ResponseField};
use crate::Result;
use crate::{api_defaults, error};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use ureq::Error;

pub struct Client<C, D> {
    cache: C,
    config: D,
    refresh_cache: bool,
}

impl<C, D> Client<C, D> {
    pub fn new(cache: C, config: D, refresh_cache: bool) -> Self {
        Client {
            cache,
            refresh_cache,
            config,
        }
    }

    fn get<T>(&self, request: &Request<T>) -> Result<Response> {
        // set incoming requests headers
        let ureq_req = ureq::get(request.url());
        let ureq_req = request
            .headers()
            .iter()
            .fold(ureq_req, |req, (key, value)| req.set(key, value));
        match ureq_req.call() {
            Ok(response) => {
                let status = response.status().into();
                // Grab headers for pagination and cache.
                let headers =
                    response
                        .headers_names()
                        .iter()
                        .fold(HashMap::new(), |mut headers, name| {
                            headers.insert(
                                name.to_lowercase(),
                                response.header(name.as_str()).unwrap().to_string(),
                            );
                            headers
                        });
                let body = response.into_string().unwrap();
                let response = Response::new()
                    .with_status(status)
                    .with_body(body)
                    .with_headers(headers);
                Ok(response)
            }
            Err(err) => Err(err.into()),
        }
    }

    fn update_create<T: Serialize>(
        &self,
        request: &Request<T>,
        ureq_req: ureq::Request,
    ) -> Result<Response> {
        let ureq_req = request
            .headers()
            .iter()
            .fold(ureq_req, |req, (key, value)| req.set(key, value));
        match ureq_req.send_json(serde_json::to_value(&request.body).unwrap()) {
            Ok(response) => {
                let status = response.status().into();
                let body = response.into_string().unwrap();
                let response = Response::new().with_status(status).with_body(body);
                Ok(response)
            }
            Err(Error::Status(code, response)) => {
                // ureq returns error on status codes >= 400
                // so we need to handle this case separately
                // https://docs.rs/ureq/latest/ureq/#error-handling
                let status = code.into();
                let body = response.into_string().unwrap();
                let response = Response::new().with_status(status).with_body(body);
                Ok(response)
            }
            Err(err) => Err(err.into()),
        }
    }

    fn post<T: Serialize>(&self, request: &Request<T>) -> Result<Response> {
        let ureq_req = ureq::post(request.url());
        self.update_create(request, ureq_req)
    }

    fn patch<T: Serialize>(&self, request: &Request<T>) -> Result<Response> {
        let ureq_req = ureq::patch(request.url());
        self.update_create(request, ureq_req)
    }

    fn put<T: Serialize>(&self, request: &Request<T>) -> Result<Response> {
        let ureq_req = ureq::put(request.url());
        self.update_create(request, ureq_req)
    }
}

impl<C, D: ConfigProperties> Client<C, D> {
    fn handle_rate_limit(&self, response: &mut Response) -> Result<()> {
        let headers = response.get_ratelimit_headers();
        if headers.remaining <= self.config.rate_limit_remaining_threshold() {
            return Err(error::GRError::RateLimitExceeded(
                "Rate limit threshold reached".to_string(),
            )
            .into());
        }
        Ok(())
    }
}

pub struct Resource {
    pub url: String,
    pub api_operation: Option<ApiOperation>,
}

impl Resource {
    fn new(url: &str, api_operation: Option<ApiOperation>) -> Self {
        Resource {
            url: url.to_string(),
            api_operation,
        }
    }
}

pub struct Request<T> {
    body: Option<T>,
    headers: HashMap<String, String>,
    method: Method,
    pub resource: Resource,
}

impl<T> Request<T> {
    pub fn new(url: &str, method: Method) -> Self {
        Request {
            body: None,
            headers: HashMap::new(),
            method,
            resource: Resource::new(url, None),
        }
    }

    pub fn with_api_operation(mut self, api_operation: ApiOperation) -> Self {
        self.resource.api_operation = Some(api_operation);
        self
    }

    pub fn api_operation(&self) -> &Option<ApiOperation> {
        &self.resource.api_operation
    }

    pub fn with_body(mut self, body: T) -> Self {
        self.body = Some(body);
        self
    }

    pub fn set_header(&mut self, key: &str, value: &str) {
        self.headers.insert(key.to_string(), value.to_string());
    }

    pub fn set_headers(&mut self, headers: HashMap<String, String>) {
        self.headers = headers;
    }

    pub fn set_url(&mut self, url: &str) {
        self.resource.url = url.to_string();
    }

    pub fn url(&self) -> &str {
        &self.resource.url
    }

    pub fn headers(&self) -> &HashMap<String, String> {
        &self.headers
    }
}

pub enum Method {
    GET,
    POST,
    PUT,
    PATCH,
}

impl<C: Cache<Resource>, D: ConfigProperties> HttpRunner for Client<C, D> {
    type Response = Response;

    fn run<T: Serialize>(&self, cmd: &mut Request<T>) -> Result<Self::Response> {
        match cmd.method {
            Method::GET => {
                let mut default_response = Response::new();
                if !self.refresh_cache {
                    match self.cache.get(&cmd.resource) {
                        Ok(CacheState::Fresh(response)) => return Ok(response),
                        Ok(CacheState::Stale(response)) => {
                            default_response = response;
                        }
                        Ok(CacheState::None) => {}
                        Err(err) => return Err(err),
                    }
                }
                // check Etag is available in the default response.
                // If so, then we need to set the If-None-Match header.
                if let Some(etag) = default_response.get_etag() {
                    cmd.set_header("If-None-Match", etag);
                }
                // If status is 304, then we need to return the cached response.
                let mut response = self.get(cmd)?;
                if response.status() == 304 {
                    // Update cache with latest headers. This effectively
                    // refreshes the cache and we won't hit this until per api
                    // cache expiration as declared in the config.
                    self.cache
                        .update(&cmd.resource, &response, &ResponseField::Headers)?;
                    return Ok(default_response);
                }
                self.handle_rate_limit(&mut response)?;
                self.cache.set(&cmd.resource, &response).unwrap();
                Ok(response)
            }
            Method::POST => {
                let mut response = self.post(cmd)?;
                self.handle_rate_limit(&mut response)?;
                Ok(response)
            }
            Method::PATCH => {
                let mut response = self.patch(cmd)?;
                self.handle_rate_limit(&mut response)?;
                Ok(response)
            }
            Method::PUT => {
                let mut response = self.put(cmd)?;
                self.handle_rate_limit(&mut response)?;
                Ok(response)
            }
        }
    }

    fn api_max_pages<T: Serialize>(&self, cmd: &Request<T>) -> u32 {
        let max_pages = self
            .config
            .get_max_pages(&cmd.resource.api_operation.as_ref().unwrap());
        max_pages
    }
}

pub struct Paginator<'a, R, T> {
    runner: &'a R,
    request: Request<T>,
    page_url: Option<String>,
    iter: u32,
}

impl<'a, R, T> Paginator<'a, R, T> {
    pub fn new(runner: &'a R, request: Request<T>, page_url: &str) -> Self {
        Paginator {
            runner,
            request,
            page_url: Some(page_url.to_string()),
            iter: 0,
        }
    }
}

impl<'a, T: Serialize, R: HttpRunner<Response = Response>> Iterator for Paginator<'a, Arc<R>, T> {
    type Item = Result<Response>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(page_url) = &self.page_url {
            if self.iter == self.runner.api_max_pages(&self.request) {
                return None;
            }
            if self.iter >= 1 {
                self.request.set_url(page_url);
            }
            match self.runner.run(&mut self.request) {
                Ok(response) => {
                    if let Some(page_headers) = response.get_page_headers() {
                        let next_page = page_headers.next;
                        let last_page = page_headers.last;
                        match (next_page, last_page) {
                            (Some(next), _) => self.page_url = Some(next.url),
                            (None, _) => self.page_url = None,
                        }
                        self.iter += 1;
                        return Some(Ok(response));
                    }
                    self.page_url = None;
                    return Some(Ok(response));
                }
                Err(err) => {
                    self.page_url = None;
                    return Some(Err(err));
                }
            }
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
        test::utils::{ConfigMock, MockRunner},
    };

    fn header_processor_next_page_no_last(_header: &str) -> PageHeader {
        let mut page_header = PageHeader::new();
        page_header.set_next_page(Page::new("http://localhost?page=2", 1));
        page_header
    }

    fn header_processor_last_page_no_next(_header: &str) -> PageHeader {
        let mut page_header = PageHeader::new();
        page_header.set_last_page(Page::new("http://localhost?page=2", 1));
        page_header
    }

    fn response_with_next_page() -> Response {
        let mut headers = HashMap::new();
        headers.insert("link".to_string(), "http://localhost?page=2".to_string());
        let response1 = Response::new()
            .with_status(200)
            .with_headers(headers)
            .with_header_processor(header_processor_next_page_no_last);
        response1
    }

    fn response_with_last_page() -> Response {
        let mut headers = HashMap::new();
        headers.insert("link".to_string(), "http://localhost?page=2".to_string());
        let response1 = Response::new()
            .with_status(200)
            .with_headers(headers)
            .with_header_processor(header_processor_last_page_no_next);
        response1
    }

    #[test]
    fn test_paginator_no_headers_no_next_no_last_pages() {
        let response = Response::new().with_status(200);
        let client = Arc::new(MockRunner::new(vec![response]));
        let request: Request<()> = Request::new("http://localhost", Method::GET);
        let paginator = Paginator::new(&client, request, "http://localhost");
        let responses = paginator.collect::<Vec<Result<Response>>>();
        assert_eq!(1, responses.len());
        assert_eq!("http://localhost", *client.url());
    }

    #[test]
    fn test_paginator_with_link_headers_one_next_and_no_last_pages() {
        let response1 = response_with_next_page();
        let mut headers = HashMap::new();
        headers.insert("link".to_string(), "http://localhost?page=2".to_string());
        let response2 = Response::new()
            .with_status(200)
            .with_headers(headers)
            .with_header_processor(|_header| PageHeader::new());
        let client = Arc::new(MockRunner::new(vec![response2, response1]));
        let request: Request<()> = Request::new("http://localhost", Method::GET);
        let paginator = Paginator::new(&client, request, "http://localhost");
        let responses = paginator.collect::<Vec<Result<Response>>>();
        assert_eq!(2, responses.len());
    }

    #[test]
    fn test_paginator_with_link_headers_one_next_and_one_last_pages() {
        let response1 = response_with_next_page();
        let response2 = response_with_last_page();
        let client = Arc::new(MockRunner::new(vec![response2, response1]));
        let request: Request<()> = Request::new("http://localhost", Method::GET);
        let paginator = Paginator::new(&client, request, "http://localhost");
        let responses = paginator.collect::<Vec<Result<Response>>>();
        assert_eq!(2, responses.len());
    }

    #[test]
    fn test_paginator_error_response() {
        let response = Response::new()
            .with_status(500)
            .with_body("Internal Server Error".to_string());
        let client = Arc::new(MockRunner::new(vec![response]));
        let request: Request<()> = Request::new("http://localhost", Method::GET);
        let paginator = Paginator::new(&client, request, "http://localhost");
        let responses = paginator.collect::<Vec<Result<Response>>>();
        assert_eq!(1, responses.len());
        assert_eq!("http://localhost", *client.url());
    }

    #[test]
    fn test_client_get_api_max_pages() {
        let config = ConfigMock::new(1);
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
        let paginator = Paginator::new(&client, request, "http://localhost");
        let responses = paginator.collect::<Vec<Result<Response>>>();
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
        let paginator = Paginator::new(&client, request, "http://localhost");
        let responses = paginator.collect::<Vec<Result<Response>>>();
        assert_eq!(REST_API_MAX_PAGES, responses.len() as u32);
    }

    #[test]
    fn test_ratelimit_remaining_threshold_reached_is_error() {
        let mut headers = HashMap::new();
        headers.insert("x-ratelimit-remaining".to_string(), "10".to_string());
        let mut response = Response::new().with_status(200).with_headers(headers);
        let client = Client::new(cache::NoCache, ConfigMock::new(1), false);
        assert!(client.handle_rate_limit(&mut response).is_err());
    }

    #[test]
    fn test_ratelimit_remaining_threshold_not_reached_is_ok() {
        let mut headers = HashMap::new();
        headers.insert("ratelimit-remaining".to_string(), "11".to_string());
        let mut response = Response::new().with_status(200).with_headers(headers);
        let client = Client::new(cache::NoCache, ConfigMock::new(1), false);
        assert!(client.handle_rate_limit(&mut response).is_ok());
    }
}
