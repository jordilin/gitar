use std::iter::Iterator;
use std::sync::Arc;

use crate::{
    api_traits::ApiOperation,
    error,
    github::GithubMemberFields,
    gitlab::{GitlabMemberFields, GitlabPipelineFields},
    http::{self, Headers, Paginator, Request},
    io::{HttpRunner, Response},
    json_load_page, json_loads,
    remote::ListBodyArgs,
    Result,
};

use super::{Member, Pipeline};

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

macro_rules! paged {
    ($func_name:ident, $map_type:ident, $return_type:ident) => {
        pub fn $func_name<R: HttpRunner<Response = Response>>(
            runner: &Arc<R>,
            url: &str,
            list_args: Option<ListBodyArgs>,
            request_headers: Headers,
            operation: ApiOperation,
        ) -> Result<Vec<$return_type>> {
            let mut request: http::Request<()> =
                http::Request::new(&url, http::Method::GET).with_api_operation(operation);
            request.set_headers(request_headers);
            if list_args.is_some() {
                let from_page = list_args.as_ref().unwrap().page;
                let suffix = format!("?page={}", &from_page);
                let url = format!("{}{}", url, suffix);
                request.set_max_pages(list_args.unwrap().max_pages);
                request.set_url(&url);
            }
            let paginator = Paginator::new(&runner, request, url);
            let all_data = paginator
                .map(|response| {
                    let response = response?;
                    if !response.is_ok() {
                        return Err(query_error(&url, &response).into());
                    }
                    let paged_data = json_load_page(&response.body)?.iter().fold(
                        Vec::new(),
                        |mut paged_data, data| {
                            paged_data.push(<$map_type>::from(data).into());
                            paged_data
                        },
                    );
                    Ok(paged_data)
                })
                .collect::<Result<Vec<Vec<$return_type>>>>()
                .map(|paged_data| paged_data.into_iter().flatten().collect());
            match all_data {
                Ok(paged_data) => Ok(paged_data),
                Err(err) => Err(err),
            }
        }
    };
}

paged!(github_list_members, GithubMemberFields, Member);
paged!(gitlab_list_members, GitlabMemberFields, Member);
paged!(gitlab_list_pipelines, GitlabPipelineFields, Pipeline);
