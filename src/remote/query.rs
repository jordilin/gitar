use std::iter::Iterator;
use std::sync::Arc;

use serde::Serialize;

use crate::{
    api_traits::ApiOperation,
    error,
    github::{
        GithubMemberFields, GithubMergeRequestFields, GithubPipelineFields, GithubProjectFields,
    },
    gitlab::{
        GitlabMemberFields, GitlabMergeRequestFields, GitlabPipelineFields, GitlabProjectFields,
    },
    http::{self, Body, Headers, Paginator, Request, Resource},
    io::{HttpRunner, Response},
    json_load_page, json_loads,
    remote::ListBodyArgs,
    Result,
};

use super::{Member, MergeRequestResponse, Pipeline, Project};

pub fn num_pages<R: HttpRunner<Response = Response>>(
    runner: &Arc<R>,
    url: &str,
    request_headers: Headers,
    api_operation: ApiOperation,
) -> Result<Option<u32>> {
    let mut request: Request<()> = http::Request::builder()
        .method(http::Method::GET)
        .resource(Resource::new(&url, Some(api_operation)))
        .headers(request_headers)
        .build()
        .unwrap();
    let response = runner.run(&mut request)?;
    let page_header = response
        .get_page_headers()
        .ok_or_else(|| error::gen(format!("Failed to get page headers for URL: {}", url)))?;
    if let Some(last_page) = page_header.last {
        return Ok(Some(last_page.number));
    }
    Ok(None)
}

pub fn send<R: HttpRunner<Response = Response>, T: Serialize>(
    runner: &Arc<R>,
    url: &str,
    body: Option<Body<T>>,
    request_headers: Headers,
    method: http::Method,
    operation: ApiOperation,
) -> Result<serde_json::Value> {
    let mut request = if let Some(body) = body {
        http::Request::builder()
            .method(method)
            .resource(Resource::new(&url, Some(operation)))
            .body(body)
            .headers(request_headers)
            .build()
            .unwrap()
    } else {
        http::Request::builder()
            .method(method)
            .resource(Resource::new(&url, Some(operation)))
            .headers(request_headers)
            .build()
            .unwrap()
    };
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

macro_rules! get {
    ($func_name:ident, $map_type:ident, $return_type:ident) => {
        pub fn $func_name<R: HttpRunner<Response = Response>, T: Serialize>(
            runner: &Arc<R>,
            url: &str,
            body: Option<Body<T>>,
            request_headers: Headers,
            method: http::Method,
            operation: ApiOperation,
        ) -> Result<$return_type> {
            let mut request = if let Some(body) = body {
                http::Request::builder()
                    .method(method)
                    .resource(Resource::new(&url, Some(operation)))
                    .body(body)
                    .headers(request_headers)
                    .build()
                    .unwrap()
            } else {
                http::Request::builder()
                    .method(method)
                    .resource(Resource::new(&url, Some(operation)))
                    .headers(request_headers)
                    .build()
                    .unwrap()
            };
            let response = runner.run(&mut request)?;
            if !response.is_ok() {
                return Err(query_error(&url, &response).into());
            }
            let body = json_loads(&response.body)?;
            Ok(<$map_type>::from(&body).into())
        }
    };
}

macro_rules! paged {
    ($func_name:ident, $map_type:ident, $return_type:ident) => {
        pub fn $func_name<R: HttpRunner<Response = Response>>(
            runner: &Arc<R>,
            url: &str,
            list_args: Option<ListBodyArgs>,
            request_headers: Headers,
            iter_over_sub_array: Option<&str>,
            operation: ApiOperation,
        ) -> Result<Vec<$return_type>> {
            let mut request: http::Request<()> =
                http::Request::new(&url, http::Method::GET).with_api_operation(operation);
            request.set_headers(request_headers);
            if list_args.is_some() {
                let from_page = list_args.as_ref().unwrap().page;
                let url = if url.contains('?') {
                    format!("{}&page={}", url, &from_page)
                } else {
                    format!("{}?page={}", url, &from_page)
                };
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
                    if iter_over_sub_array.is_some() {
                        let body = json_loads(&response.body)?;
                        let paged_data = body[iter_over_sub_array.unwrap()]
                            .as_array()
                            .ok_or_else(|| {
                                error::GRError::RemoteUnexpectedResponseContract(format!(
                                    "Expected an array of {} but got: {}",
                                    iter_over_sub_array.unwrap(),
                                    response.body
                                ))
                            })?
                            .iter()
                            .fold(Vec::new(), |mut paged_data, data| {
                                paged_data.push(<$map_type>::from(data).into());
                                paged_data
                            });
                        return Ok(paged_data);
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
paged!(github_list_pipelines, GithubPipelineFields, Pipeline);
paged!(gitlab_list_pipelines, GitlabPipelineFields, Pipeline);
paged!(
    github_list_merge_requests,
    GithubMergeRequestFields,
    MergeRequestResponse
);
paged!(
    gitlab_list_merge_requests,
    GitlabMergeRequestFields,
    MergeRequestResponse
);

get!(gitlab_project_data, GitlabProjectFields, Project);
get!(github_project_data, GithubProjectFields, Project);
get!(
    github_get_merge_request,
    GithubMergeRequestFields,
    MergeRequestResponse
);
get!(
    gitlab_get_merge_request,
    GitlabMergeRequestFields,
    MergeRequestResponse
);
