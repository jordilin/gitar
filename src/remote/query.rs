use std::sync::Arc;

use crate::{
    api_traits::ApiOperation,
    error,
    http::{self, Headers, Request},
    io::{HttpRunner, Response},
    Result,
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
