use std::sync::Arc;

use crate::{
    api_traits::ApiOperation,
    error,
    http::{self, Headers, Request},
    io::{HttpRunner, Response},
    json_loads, Result,
};

pub fn num_pages<R: HttpRunner<Response = Response>>(
    runner: &Arc<R>,
    url: &str,
    request_headers: Headers,
    operation: ApiOperation,
) -> Result<Option<u32>> {
    let mut request: Request<()> =
        Request::new(&url, http::Method::GET).with_api_operation(operation);
    request.set_headers(request_headers);
    let response = runner.run(&mut request)?;
    let page_header = response
        .get_page_headers()
        .ok_or_else(|| error::gen(format!("Failed to get page headers for URL: {}", url)))?;
    if let Some(last_page) = page_header.last {
        return Ok(Some(last_page.number));
    }
    Ok(None)
}

pub fn send<R: HttpRunner<Response = Response>>(
    runner: &Arc<R>,
    url: &str,
    request_headers: Headers,
    method: http::Method,
    operation: ApiOperation,
) -> Result<serde_json::Value> {
    let mut request: Request<()> = Request::new(&url, method).with_api_operation(operation);
    request.set_headers(request_headers);
    let response = runner.run(&mut request)?;
    if !response.is_ok() {
        return Err(error::gen(format!(
            "Failed to submit request query to URL: {} with status code: {} and body: {}",
            url, response.status, response.body
        )));
    }
    json_loads(&response.body)
}
