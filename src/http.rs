use crate::api_defaults::REST_API_MAX_PAGES;
use crate::api_traits::ApiOperation;
use crate::cache::{Cache, CacheState};
use crate::io::{HttpRunner, Response};
use crate::Result;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use ureq::Error;

pub struct Client<C> {
    cache: C,
    refresh_cache: bool,
}

impl<C> Client<C> {
    pub fn new(cache: C, refresh_cache: bool) -> Self {
        Client {
            cache,
            refresh_cache,
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
                                name.to_string(),
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

pub struct Resource {
    pub url: String,
    api_operation: Option<ApiOperation>,
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

impl<C: Cache<Resource>> HttpRunner for Client<C> {
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
                let response = self.get(cmd)?;
                if response.status() == 304 {
                    // Not modified return the cached response.
                    return Ok(default_response);
                }
                self.cache.set(&cmd.resource, &response).unwrap();
                Ok(response)
            }
            Method::POST => {
                let response = self.post(cmd)?;
                Ok(response)
            }
            Method::PATCH => {
                let response = self.patch(cmd)?;
                Ok(response)
            }
            Method::PUT => {
                let response = self.put(cmd)?;
                Ok(response)
            }
        }
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
            if self.iter == REST_API_MAX_PAGES {
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
        io::{Page, PageHeader},
        test::utils::MockRunner,
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
    fn test_paginator_three_pages_limits_to_max_pages() {
        let response1 = response_with_next_page();
        let response2 = response_with_next_page();
        let response3 = response_with_last_page();
        let client = Arc::new(MockRunner::new(vec![response3, response2, response1]));
        let request: Request<()> = Request::new("http://localhost", Method::GET);
        let paginator = Paginator::new(&client, request, "http://localhost");
        let responses = paginator.collect::<Vec<Result<Response>>>();
        assert_eq!(2, responses.len());
    }
}
