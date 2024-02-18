use super::Github;
use crate::{
    api_traits::{ApiOperation, MergeRequest, RemoteProject},
    cli::BrowseOptions,
    http::{
        Body,
        Method::{GET, PATCH, POST, PUT},
    },
    io::{HttpRunner, Response},
    json_loads,
    remote::{
        query, MergeRequestBodyArgs, MergeRequestListBodyArgs, MergeRequestResponse,
        MergeRequestState,
    },
};

use crate::{error, Result};

impl<R> Github<R> {
    fn url_list_merge_requests(&self, args: &MergeRequestListBodyArgs) -> String {
        match args.state {
            MergeRequestState::Opened => {
                format!(
                    "{}/repos/{}/pulls?state=open",
                    self.rest_api_basepath, self.path
                )
            }
            // Github has no distinction between closed and merged. A merged
            // pull request is considered closed.
            MergeRequestState::Closed | MergeRequestState::Merged => {
                format!(
                    "{}/repos/{}/pulls?state=closed",
                    self.rest_api_basepath, self.path
                )
            }
        }
    }
}

impl<R: HttpRunner<Response = Response>> MergeRequest for Github<R> {
    fn open(&self, args: MergeRequestBodyArgs) -> Result<MergeRequestResponse> {
        let mut body = Body::new();
        body.add("head", args.source_branch.clone());
        body.add("base", args.target_branch);
        body.add("title", args.title);
        body.add("body", args.description);
        // Add draft in payload only when requested. It seems that Github opens
        // PR in draft mode even when the draft value is false.
        if args.draft {
            body.add("draft", args.draft.to_string());
        }
        let mr_url = format!("{}/repos/{}/pulls", self.rest_api_basepath, self.path);
        match query::github_merge_request_response(
            &self.runner,
            &mr_url,
            Some(body),
            self.request_headers(),
            POST,
            ApiOperation::MergeRequest,
        ) {
            Ok(response) => {
                let body = response.body;
                match response.status {
                    201 => {
                        // This is a new pull request
                        // Set the assignee to the pull request. Currently, the
                        // only way to set the assignee to a pull request is by
                        // using the issues API. All pull requests in Github API v3
                        // are considered to be issues, but not the other way
                        // around.
                        // See note in https://docs.github.com/en/rest/issues/issues#list-repository-issues
                        // Note: Github's REST API v3 considers every pull request
                        // an issue, but not every issue is a pull request.
                        // https://docs.github.com/en/rest/issues/issues#update-an-issue
                        let merge_request_json = json_loads(&body)?;
                        let id = merge_request_json["number"].to_string();
                        let issues_url = format!(
                            "{}/repos/{}/issues/{}",
                            self.rest_api_basepath, self.path, id
                        );
                        let mut body = Body::new();
                        let assignees = vec![args.username.as_str()];
                        body.add("assignees", &assignees);
                        query::github_merge_request::<_, &Vec<&str>>(
                            &self.runner,
                            &issues_url,
                            Some(body),
                            self.request_headers(),
                            PATCH,
                            ApiOperation::MergeRequest,
                        )
                    }
                    422 => {
                        // There is an existing pull request already.
                        // Gather its URL by querying Github pull requests filtering by
                        // namespace:branch
                        let remote_pr_branch = format!("{}:{}", self.path, args.source_branch);
                        let existing_mr_url = format!("{}?head={}", mr_url, remote_pr_branch);
                        let response = query::github_merge_request_response::<_, ()>(
                            &self.runner,
                            &existing_mr_url,
                            None,
                            self.request_headers(),
                            GET,
                            ApiOperation::MergeRequest,
                        )?;
                        let merge_requests_json: Vec<serde_json::Value> =
                            serde_json::from_str(&response.body)?;
                        if merge_requests_json.len() == 1 {
                            Ok(MergeRequestResponse::builder()
                                .id(merge_requests_json[0]["id"].as_i64().unwrap())
                                .web_url(
                                    merge_requests_json[0]["html_url"]
                                        .to_string()
                                        .trim_matches('"')
                                        .to_string(),
                                )
                                .build()
                                .unwrap())
                        } else {
                            Err(error::GRError::RemoteUnexpectedResponseContract(format!(
                                "There should have been an existing pull request at \
                                URL: {} but got an unexpected response: {}",
                                existing_mr_url, response.body
                            ))
                            .into())
                        }
                    }
                    _ => Err(error::gen(format!(
                        "Failed to create merge request. Status code: {}, Body: {}",
                        response.status, body
                    ))),
                }
            }
            Err(err) => Err(err),
        }
    }

    fn list(&self, args: MergeRequestListBodyArgs) -> Result<Vec<MergeRequestResponse>> {
        let mut url = self.url_list_merge_requests(&args);
        query::github_list_merge_requests(
            &self.runner,
            &mut url,
            args.list_args,
            self.request_headers(),
            None,
            ApiOperation::MergeRequest,
        )
    }

    fn merge(&self, id: i64) -> Result<MergeRequestResponse> {
        // https://docs.github.com/en/rest/pulls/pulls?apiVersion=2022-11-28#merge-a-pull-request
        //  /repos/{owner}/{repo}/pulls/{pull_number}/merge
        let url = format!(
            "{}/repos/{}/pulls/{}/merge",
            self.rest_api_basepath, self.path, id
        );
        query::github_merge_request_json::<_, ()>(
            &self.runner,
            &url,
            None,
            self.request_headers(),
            PUT,
            ApiOperation::MergeRequest,
        )?;
        // Response:
        // {
        //     "sha": "6dcb09b5b57875f334f61aebed695e2e4193db5e",
        //     "merged": true,
        //     "message": "Pull Request successfully merged"
        // }

        // We do not have the id nor the url available in the response. Compute
        // it and return it to the client so we can open the url if needed.
        Ok(MergeRequestResponse::builder()
            .id(id)
            .web_url(self.get_url(BrowseOptions::MergeRequestId(id)))
            .build()
            .unwrap())
    }

    fn get(&self, id: i64) -> Result<MergeRequestResponse> {
        let url = format!(
            "{}/repos/{}/pulls/{}",
            self.rest_api_basepath, self.path, id
        );
        query::github_merge_request::<_, ()>(
            &self.runner,
            &url,
            None,
            self.request_headers(),
            GET,
            ApiOperation::MergeRequest,
        )
    }

    fn close(&self, _id: i64) -> Result<MergeRequestResponse> {
        todo!()
    }

    fn num_pages(&self, args: MergeRequestListBodyArgs) -> Result<Option<u32>> {
        let url = self.url_list_merge_requests(&args) + "&page=1";
        let headers = self.request_headers();
        query::num_pages(&self.runner, &url, headers, ApiOperation::MergeRequest)
    }
}

pub struct GithubMergeRequestFields {
    id: i64,
    web_url: String,
    source_branch: String,
    author: String,
    updated_at: String,
}

impl From<&serde_json::Value> for GithubMergeRequestFields {
    fn from(merge_request_data: &serde_json::Value) -> Self {
        GithubMergeRequestFields {
            id: merge_request_data["number"].as_i64().unwrap(),
            web_url: merge_request_data["html_url"].as_str().unwrap().to_string(),
            source_branch: merge_request_data["head"]["ref"]
                .as_str()
                .unwrap()
                .to_string(),
            author: merge_request_data["user"]["login"]
                .as_str()
                .unwrap()
                .to_string(),
            updated_at: merge_request_data["updated_at"]
                .as_str()
                .unwrap()
                .to_string(),
        }
    }
}

impl From<GithubMergeRequestFields> for MergeRequestResponse {
    fn from(fields: GithubMergeRequestFields) -> Self {
        MergeRequestResponse::builder()
            .id(fields.id)
            .web_url(fields.web_url)
            .source_branch(fields.source_branch)
            .author(fields.author)
            .updated_at(fields.updated_at)
            .build()
            .unwrap()
    }
}

#[cfg(test)]
mod test {

    use std::sync::Arc;

    use crate::{
        http::Headers,
        remote::{ListBodyArgs, MergeRequestState},
        test::utils::{config, get_contract, ContractType, MockRunner},
    };

    use super::*;

    #[test]
    fn test_open_merge_request() {
        let config = config();
        let mr_args = MergeRequestBodyArgs::builder().build().unwrap();

        let domain = "github.com".to_string();
        let path = "jordilin/githapi";
        let response1 = Response::builder()
            .status(201)
            .body(get_contract(ContractType::Github, "merge_request.json"))
            .build()
            .unwrap();
        let response2 = Response::builder()
            .status(200)
            .body(get_contract(ContractType::Github, "merge_request.json"))
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response2, response1]));
        let github = Github::new(config, &domain, &path, client.clone());

        assert!(github.open(mr_args).is_ok());
        assert_eq!(
            "https://api.github.com/repos/jordilin/githapi/issues/23",
            *client.url(),
        );
        assert_eq!(
            Some(ApiOperation::MergeRequest),
            *client.api_operation.borrow()
        );
    }

    #[test]
    fn test_open_merge_request_error_status_code() {
        let config = config();
        let mr_args = MergeRequestBodyArgs::builder().build().unwrap();

        let domain = "github.com".to_string();
        let path = "jordilin/githapi";
        let response1 = Response::builder().status(401).body(
            r#"{"message":"Bad credentials","documentation_url":"https://docs.github.com/rest"}"#
                .to_string(),
            )
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response1]));
        let github = Github::new(config, &domain, &path, client.clone());
        assert!(github.open(mr_args).is_err());
    }

    #[test]
    fn test_open_merge_request_existing_one() {
        let config = config();
        let mr_args = MergeRequestBodyArgs::builder()
            .source_branch("feature".to_string())
            .build()
            .unwrap();

        let domain = "github.com".to_string();
        let path = "jordilin/githapi";
        let response1 = Response::builder()
            .status(422)
            .body(get_contract(
                ContractType::Github,
                "merge_request_conflict.json",
            ))
            .build()
            .unwrap();
        // Github returns a 422 (already exists), so the code grabs existing URL
        // filtering by namespace and branch. The response is a list of merge
        // requests.
        let response2 = Response::builder()
            .status(200)
            .body(format!(
                "[{}]",
                get_contract(ContractType::Github, "merge_request.json")
            ))
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response2, response1]));
        let github = Github::new(config, &domain, &path, client.clone());

        github.open(mr_args).unwrap();
        assert_eq!(
            "https://api.github.com/repos/jordilin/githapi/pulls?head=jordilin/githapi:feature",
            *client.url(),
        );
        assert_eq!(
            Some(ApiOperation::MergeRequest),
            *client.api_operation.borrow()
        );
    }

    #[test]
    fn test_open_merge_request_cannot_retrieve_url_existing_one_is_error() {
        let config = config();
        let mr_args = MergeRequestBodyArgs::builder()
            .source_branch("feature".to_string())
            .build()
            .unwrap();

        let domain = "github.com".to_string();
        let path = "jordilin/githapi";
        let response1 = Response::builder()
            .status(422)
            .body(get_contract(
                ContractType::Github,
                "merge_request_conflict.json",
            ))
            .build()
            .unwrap();
        let response2 = Response::builder()
            .status(200)
            .body("[]".to_string())
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response2, response1]));
        let github = Github::new(config, &domain, &path, client.clone());

        let result = github.open(mr_args);
        match result {
            Ok(_) => panic!("Expected error"),
            Err(err) => match err.downcast_ref::<error::GRError>() {
                Some(error::GRError::RemoteUnexpectedResponseContract(_)) => (),
                _ => panic!("Expected error::GRError::RemoteUnexpectedResponseContract"),
            },
        }
    }
    #[test]
    fn test_merge_request_num_pages() {
        let config = config();
        let domain = "github.com".to_string();
        let path = "jordilin/githapi";
        let link_header = r#"<https://api.github.com/repos/jordilin/githapi/pulls?state=open&page=2>; rel="next", <https://api.github.com/repos/jordilin/githapi/pulls?state=open&page=2>; rel="last""#;
        let mut headers = Headers::new();
        headers.set("link".to_string(), link_header.to_string());
        let response = Response::builder()
            .status(200)
            .headers(headers)
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let github: Box<dyn MergeRequest> =
            Box::new(Github::new(config, &domain, &path, client.clone()));
        let args = MergeRequestListBodyArgs::builder()
            .state(MergeRequestState::Opened)
            .list_args(None)
            .build()
            .unwrap();
        assert_eq!(Some(2), github.num_pages(args).unwrap());
        assert_eq!(
            "https://api.github.com/repos/jordilin/githapi/pulls?state=open&page=1",
            *client.url(),
        );
        assert_eq!(
            Some(ApiOperation::MergeRequest),
            *client.api_operation.borrow()
        );
    }

    #[test]
    fn test_list_merge_requests_from_to_page_set_in_url() {
        let config = config();
        let domain = "github.com".to_string();
        let path = "jordilin/githapi";
        let response = Response::builder()
            .status(200)
            .body("[]".to_string())
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let github: Box<dyn MergeRequest> =
            Box::new(Github::new(config, &domain, &path, client.clone()));
        let args = MergeRequestListBodyArgs::builder()
            .state(MergeRequestState::Opened)
            .list_args(Some(
                ListBodyArgs::builder()
                    .page(2)
                    .max_pages(3)
                    .build()
                    .unwrap(),
            ))
            .build()
            .unwrap();
        github.list(args).unwrap();
        assert_eq!(
            "https://api.github.com/repos/jordilin/githapi/pulls?state=open&page=2",
            *client.url(),
        );
        assert_eq!(
            Some(ApiOperation::MergeRequest),
            *client.api_operation.borrow()
        );
    }
}
