use std::iter::Iterator;
use std::sync::Arc;

use serde::Serialize;

use crate::{
    api_defaults,
    api_traits::{ApiOperation, NumberDeltaErr},
    cmds::{
        cicd::{Pipeline, Runner, RunnerMetadata},
        docker::{ImageMetadata, RegistryRepository, RepositoryTag},
        gist::Gist,
        merge_request::Comment,
        release::{Release, ReleaseAssetMetadata},
    },
    display, error,
    github::{
        cicd::GithubPipelineFields,
        gist::GithubGistFields,
        merge_request::{GithubMergeRequestCommentFields, GithubMergeRequestFields},
        project::{GithubMemberFields, GithubProjectFields},
        release::{GithubReleaseAssetFields, GithubReleaseFields},
        user::GithubUserFields,
    },
    gitlab::{
        cicd::{GitlabPipelineFields, GitlabRunnerFields, GitlabRunnerMetadataFields},
        container_registry::{
            GitlabImageMetadataFields, GitlabRegistryRepositoryFields, GitlabRepositoryTagFields,
        },
        merge_request::{GitlabMergeRequestCommentFields, GitlabMergeRequestFields},
        project::{GitlabMemberFields, GitlabProjectFields},
        release::GitlabReleaseFields,
        user::GitlabUserFields,
    },
    http::{self, Body, Headers, Paginator, Request, Resource},
    io::{HttpRunner, PageHeader, Response},
    json_load_page, json_loads,
    remote::ListBodyArgs,
    time::sort_filter_by_date,
    Result,
};

use super::{Member, MergeRequestResponse, Project};

fn get_page_header<R: HttpRunner<Response = Response>>(
    runner: &Arc<R>,
    url: &str,
    request_headers: Headers,
    api_operation: ApiOperation,
) -> Result<Option<PageHeader>> {
    let response = send_request::<_, String>(
        runner,
        url,
        None,
        request_headers,
        http::Method::HEAD,
        api_operation,
    )?;
    Ok(response.get_page_headers())
}

pub fn num_pages<R: HttpRunner<Response = Response>>(
    runner: &Arc<R>,
    url: &str,
    request_headers: Headers,
    api_operation: ApiOperation,
) -> Result<Option<u32>> {
    let page_header = get_page_header(runner, url, request_headers, api_operation)?;
    match page_header {
        Some(page_header) => {
            if let Some(last_page) = page_header.last {
                return Ok(Some(last_page.number));
            }
            Ok(None)
        }
        // Github does not return page headers when there is only one page, so
        // we assume 1 page in this case.
        None => Ok(Some(1)),
    }
}

pub fn num_resources<R: HttpRunner<Response = Response>>(
    runner: &Arc<R>,
    url: &str,
    request_headers: Headers,
    api_operation: ApiOperation,
) -> Result<Option<NumberDeltaErr>> {
    let page_header = get_page_header(runner, url, request_headers, api_operation)?;
    match page_header {
        Some(page_header) => {
            // total resources per_page * total_pages
            if let Some(last_page) = page_header.last {
                let count = last_page.number * page_header.per_page;
                return Ok(Some(NumberDeltaErr {
                    num: count,
                    delta: page_header.per_page,
                }));
            }
            Ok(None)
        }
        None => {
            // Github does not return page headers when there is only one page, so
            // we assume 1 page in this case.
            Ok(Some(NumberDeltaErr {
                num: 1,
                delta: api_defaults::DEFAULT_PER_PAGE,
            }))
        }
    }
}

fn query_error(url: &str, response: &Response) -> error::GRError {
    error::GRError::RemoteServerError(format!(
        "Failed to submit request to URL: {} with status code: {} and body: {}",
        url, response.status, response.body
    ))
}

macro_rules! send {
    ($func_name:ident, $map_type:ident, $return_type:ident) => {
        pub fn $func_name<R: HttpRunner<Response = Response>, T: Serialize>(
            runner: &Arc<R>,
            url: &str,
            body: Option<Body<T>>,
            request_headers: Headers,
            method: http::Method,
            operation: ApiOperation,
        ) -> Result<$return_type> {
            let response = send_request(runner, url, body, request_headers, method, operation)?;
            let body = json_loads(&response.body)?;
            Ok(<$map_type>::from(&body).into())
        }
    };
    ($func_name:ident, Response) => {
        pub fn $func_name<R: HttpRunner<Response = Response>, T: Serialize>(
            runner: &Arc<R>,
            url: &str,
            body: Option<Body<T>>,
            request_headers: Headers,
            method: http::Method,
            operation: ApiOperation,
        ) -> Result<Response> {
            send_request(runner, url, body, request_headers, method, operation)
        }
    };
    ($func_name:ident, serde_json::Value) => {
        pub fn $func_name<R: HttpRunner<Response = Response>, T: Serialize>(
            runner: &Arc<R>,
            url: &str,
            body: Option<Body<T>>,
            request_headers: Headers,
            method: http::Method,
            operation: ApiOperation,
        ) -> Result<serde_json::Value> {
            let response = send_request(runner, url, body, request_headers, method, operation)?;
            json_loads(&response.body)
        }
    };
}

fn send_request<R: HttpRunner<Response = Response>, T: Serialize>(
    runner: &Arc<R>,
    url: &str,
    body: Option<Body<T>>,
    request_headers: Headers,
    method: http::Method,
    operation: ApiOperation,
) -> Result<Response> {
    let mut request = if let Some(body) = body {
        http::Request::builder()
            .method(method.clone())
            .resource(Resource::new(url, Some(operation)))
            .body(body)
            .headers(request_headers)
            .build()
            .unwrap()
    } else {
        http::Request::builder()
            .method(method.clone())
            .resource(Resource::new(url, Some(operation)))
            .headers(request_headers)
            .build()
            .unwrap()
    };
    let response = runner.run(&mut request)?;
    if !response.is_ok(&method) {
        return Err(query_error(url, &response).into());
    }
    Ok(response)
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
            let request = build_list_request(url, &list_args, request_headers, operation);
            let mut throttle_time = None;
            let mut backoff_max_retries = 0;
            let mut backoff_wait_time = 60;
            if let Some(list_args) = &list_args {
                throttle_time = list_args.throttle_time;
                backoff_max_retries = list_args.get_args.backoff_max_retries;
                backoff_wait_time = list_args.get_args.backoff_retry_after;
            }
            let paginator = Paginator::new(
                &runner,
                request,
                url,
                throttle_time,
                backoff_max_retries,
                backoff_wait_time,
            );
            let all_data = paginator
                .map(|response| {
                    let response = response?;
                    if !response.is_ok(&http::Method::GET) {
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
                        if let Some(list_args) = &list_args {
                            if list_args.flush {
                                display::print(
                                    &mut std::io::stdout(),
                                    paged_data,
                                    list_args.get_args.clone(),
                                )
                                .unwrap();
                                return Ok(Vec::new());
                            }
                        }
                        return Ok(paged_data);
                    }
                    let paged_data = json_load_page(&response.body)?.iter().fold(
                        Vec::new(),
                        |mut paged_data, data| {
                            paged_data.push(<$map_type>::from(data).into());
                            paged_data
                        },
                    );
                    if let Some(list_args) = &list_args {
                        if list_args.flush {
                            display::print(
                                &mut std::io::stdout(),
                                paged_data,
                                list_args.get_args.clone(),
                            )
                            .unwrap();
                            return Ok(Vec::new());
                        }
                    }
                    Ok(paged_data)
                })
                .collect::<Result<Vec<Vec<$return_type>>>>()
                .map(|paged_data| paged_data.into_iter().flatten().collect());
            match all_data {
                Ok(paged_data) => Ok(sort_filter_by_date(paged_data, list_args)?),
                Err(err) => Err(err),
            }
        }
    };
}

fn build_list_request(
    url: &str,
    list_args: &Option<ListBodyArgs>,
    request_headers: Headers,
    operation: ApiOperation,
) -> Request<()> {
    let mut request: http::Request<()> =
        http::Request::new(url, http::Method::GET).with_api_operation(operation);
    request.set_headers(request_headers);
    if let Some(list_args) = list_args {
        if let Some(from_page) = list_args.page {
            let url = if url.contains('?') {
                format!("{}&page={}", url, &from_page)
            } else {
                format!("{}?page={}", url, &from_page)
            };
            request.set_max_pages(list_args.max_pages.unwrap());
            request.set_url(&url);
        }
    }
    request
}

// Paged HTTP requests

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

paged!(
    gitlab_project_registry_repositories,
    GitlabRegistryRepositoryFields,
    RegistryRepository
);

paged!(
    gitlab_project_registry_repository_tags,
    GitlabRepositoryTagFields,
    RepositoryTag
);

paged!(github_releases, GithubReleaseFields, Release);
paged!(gitlab_releases, GitlabReleaseFields, Release);

paged!(
    github_release_assets,
    GithubReleaseAssetFields,
    ReleaseAssetMetadata
);

paged!(gitlab_list_project_runners, GitlabRunnerFields, Runner);

paged!(gitlab_list_projects, GitlabProjectFields, Project);
paged!(github_list_projects, GithubProjectFields, Project);

paged!(
    gitlab_list_merge_request_comments,
    GitlabMergeRequestCommentFields,
    Comment
);

paged!(
    github_list_merge_request_comments,
    GithubMergeRequestCommentFields,
    Comment
);

paged!(github_list_user_gists, GithubGistFields, Gist);

// Single HTTP requests

send!(gitlab_project_data, GitlabProjectFields, Project);
send!(github_project_data, GithubProjectFields, Project);
send!(
    github_merge_request,
    GithubMergeRequestFields,
    MergeRequestResponse
);

send!(github_merge_request_json, serde_json::Value);
send!(github_merge_request_response, Response);
send!(
    gitlab_merge_request,
    GitlabMergeRequestFields,
    MergeRequestResponse
);

send!(gitlab_merge_request_response, Response);
send!(
    gitlab_registry_image_tag_metadata,
    GitlabImageMetadataFields,
    ImageMetadata
);

send!(gitlab_auth_user, GitlabUserFields, Member);
send!(github_auth_user, GithubUserFields, Member);

send!(
    gitlab_get_runner_metadata,
    GitlabRunnerMetadataFields,
    RunnerMetadata
);

send!(create_merge_request_comment, Response);

send!(github_trending_language_projects, Response);

send!(gitlab_get_release, Response);

#[cfg(test)]
mod test {
    use crate::{io::Page, test::utils::MockRunner};

    use super::*;

    #[test]
    fn test_numpages_assume_one_if_pages_not_available() {
        let response = Response::builder().status(200).build().unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let url = "https://github.com/api/v4/projects/1/pipelines";
        let headers = Headers::new();
        let operation = ApiOperation::Pipeline;
        let num_pages = num_pages(&client, url, headers, operation).unwrap();
        assert_eq!(Some(1), num_pages);
    }

    #[test]
    fn test_numpages_error_on_404() {
        let response = Response::builder().status(404).build().unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let url = "https://github.com/api/v4/projects/1/pipelines";
        let headers = Headers::new();
        let operation = ApiOperation::Pipeline;
        assert!(num_pages(&client, url, headers, operation).is_err());
    }

    #[test]
    fn test_num_resources_assume_one_if_pages_not_available() {
        let headers = Headers::new();
        let response = Response::builder().status(200).build().unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let url = "https://github.com/api/v4/projects/1/pipelines?page=1";
        let num_resources = num_resources(&client, url, headers, ApiOperation::Pipeline).unwrap();
        assert_eq!(30, num_resources.unwrap().delta);
    }

    #[test]
    fn test_num_resources_with_last_page_and_per_page_available() {
        let mut headers = Headers::new();
        // Value doesn't matter as we are injecting the header processor
        // enforcing the last page and per_page values.
        headers.set("link", "");
        let response = Response::builder()
            .status(200)
            .headers(headers)
            .link_header_processor(|_link| {
                let mut page_header = PageHeader::new();
                let next_page =
                    Page::new("https://gitlab.com/api/v4/projects/1/pipelines?page=2", 2);
                let last_page =
                    Page::new("https://gitlab.com/api/v4/projects/1/pipelines?page=4", 4);
                page_header.set_next_page(next_page);
                page_header.set_last_page(last_page);
                page_header.per_page = 20;
                page_header
            })
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let url = "https://gitlab.com/api/v4/projects/1/pipelines?page=1";
        let num_resources = num_resources(&client, url, Headers::new(), ApiOperation::Pipeline)
            .unwrap()
            .unwrap();
        assert_eq!(80, num_resources.num);
        assert_eq!(20, num_resources.delta);
    }

    #[test]
    fn test_numresources_error_on_404() {
        let response = Response::builder().status(404).build().unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let url = "https://github.com/api/v4/projects/1/pipelines";
        let headers = Headers::new();
        let operation = ApiOperation::Pipeline;
        assert!(num_resources(&client, url, headers, operation).is_err());
    }
}
