use std::iter::Iterator;
use std::sync::Arc;

use crate::{
    api_traits::ApiOperation,
    error,
    github::GithubMemberFields,
    http::{self, Headers, Paginator, Request},
    io::{CmdInfo, HttpRunner, Response},
    json_load_page, json_loads, Result,
};

use super::Member;

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
        return Err(query_error(&url, &response).into());
    }
    json_loads(&response.body)
}

fn query_error(url: &str, response: &Response) -> error::GRError {
    error::GRError::RemoteServerError(format!(
        "Failed to submit request to URL: {} with status code: {} and body: {}",
        url, response.status, response.body
    ))
}

pub fn get_members<R: HttpRunner<Response = Response>>(
    runner: &Arc<R>,
    url: &str,
    request_headers: Headers,
    operation: ApiOperation,
) -> Result<CmdInfo> {
    let mut request: http::Request<()> =
        http::Request::new(&url, http::Method::GET).with_api_operation(operation);
    request.set_headers(request_headers);
    let paginator = Paginator::new(&runner, request, url);
    let members_data = paginator
        .map(|response| {
            let response = response?;
            if !response.is_ok() {
                return Err(query_error(&url, &response).into());
            }
            let members = json_load_page(&response.body)?.iter().fold(
                Vec::new(),
                |mut members, member_data| {
                    members.push(GithubMemberFields::from(member_data).into());
                    members
                },
            );
            Ok(members)
        })
        .collect::<Result<Vec<Vec<Member>>>>()
        .map(|members| members.into_iter().flatten().collect());
    match members_data {
        Ok(members) => Ok(CmdInfo::Members(members)),
        Err(err) => Err(err),
    }
}
