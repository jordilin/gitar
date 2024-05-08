use super::Github;
use crate::{
    api_traits::{ApiOperation, CommentMergeRequest, MergeRequest, RemoteProject},
    cli::browse::BrowseOptions,
    cmds::merge_request::{Comment, CommentMergeRequestBodyArgs, CommentMergeRequestListBodyArgs},
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
        let state = match args.state {
            MergeRequestState::Opened => "open".to_string(),
            // Github has no distinction between closed and merged. A merged
            // pull request is considered closed.
            MergeRequestState::Closed | MergeRequestState::Merged => "closed".to_string(),
        };
        if args.assignee_id.is_some() {
            return format!("{}/issues?state={}", self.rest_api_basepath, state);
        }
        format!(
            "{}/repos/{}/pulls?state={}",
            self.rest_api_basepath, self.path, state
        )
    }
}

impl<R: HttpRunner<Response = Response>> MergeRequest for Github<R> {
    fn open(&self, args: MergeRequestBodyArgs) -> Result<MergeRequestResponse> {
        let mut body = Body::new();
        body.add("base", args.target_branch);
        body.add("title", args.title);
        body.add("body", args.description);
        // Add draft in payload only when requested. It seems that Github opens
        // PR in draft mode even when the draft value is false.
        if args.draft {
            body.add("draft", args.draft.to_string());
        }
        let target_repo = args.target_repo.clone();
        let mut mr_url = format!(
            "{}/repos/{}/pulls",
            self.rest_api_basepath,
            self.path.clone()
        );
        if !target_repo.is_empty() {
            mr_url = format!(
                "{}/repos/{}/pulls",
                self.rest_api_basepath, args.target_repo
            );
            let owner_path = self.path.split('/').collect::<Vec<&str>>();
            if owner_path.len() != 2 {
                return Err(error::GRError::ApplicationError(format!(
                    "Invalid path format in git config: [{}] while attempting \
                    to retrieve existing pull request. Expected owner/repo",
                    self.path
                ))
                .into());
            }
            let remote_pr_branch = format!("{}:{}", owner_path[0], args.source_branch.clone());
            body.add("head", remote_pr_branch);
        } else {
            body.add("head", args.source_branch.clone());
        }

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
                        // If target repo is provided bypass user assignation
                        if !target_repo.is_empty() {
                            let json_value = json_loads(&body)?;
                            return Ok(MergeRequestResponse::from(GithubMergeRequestFields::from(
                                &json_value,
                            )));
                        }
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
                        // head owner:branch
                        // Ref:
                        // https://docs.github.com/en/rest/pulls/pulls?apiVersion=2022-11-28#list-pull-requests--parameters

                        // The path has owner/repo format.
                        let owner_path = self.path.split('/').collect::<Vec<&str>>();
                        if owner_path.len() != 2 {
                            return Err(error::GRError::ApplicationError(format!(
                                "Invalid path format in git config: [{}] while attempting \
                                to retrieve existing pull request. Expected owner/repo",
                                self.path
                            ))
                            .into());
                        }
                        let remote_pr_branch = format!("{}:{}", owner_path[0], args.source_branch);
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
        let url = self.url_list_merge_requests(&args);
        let response = query::github_list_merge_requests(
            &self.runner,
            &url,
            args.list_args,
            self.request_headers(),
            None,
            ApiOperation::MergeRequest,
        );
        if args.assignee_id.is_some() {
            // Pull requests for the current authenticated user.
            // Filter those reponses that have pull_request not empty See ref:
            // https://docs.github.com/en/rest/issues/issues?apiVersion=2022-11-28#list-issues-assigned-to-the-authenticated-user
            // Quoting Github's docs: Note: GitHub's REST API considers every
            // pull request an issue, but not every issue is a pull request. For
            // this reason, "Issues" endpoints may return both issues and pull
            // requests in the response. You can identify pull requests by the
            // pull_request key. Be aware that the id of a pull request returned
            // from "Issues" endpoints will be an issue id. To find out the pull
            // request id, use the "List pull requests" endpoint.
            let mut merge_requests = vec![];
            for mr in response? {
                if !mr.pull_request.is_empty() {
                    merge_requests.push(mr);
                }
            }
            return Ok(merge_requests);
        }
        response
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

    fn close(&self, id: i64) -> Result<MergeRequestResponse> {
        let url = format!(
            "{}/repos/{}/pulls/{}",
            self.rest_api_basepath, self.path, id
        );
        let mut body = Body::new();
        body.add("state", "closed");
        query::github_merge_request::<_, &str>(
            &self.runner,
            &url,
            Some(body),
            self.request_headers(),
            PATCH,
            ApiOperation::MergeRequest,
        )
    }

    fn num_pages(&self, args: MergeRequestListBodyArgs) -> Result<Option<u32>> {
        let url = self.url_list_merge_requests(&args) + "&page=1";
        let headers = self.request_headers();
        query::num_pages(&self.runner, &url, headers, ApiOperation::MergeRequest)
    }

    fn approve(&self, _id: i64) -> Result<MergeRequestResponse> {
        todo!()
    }
}

impl<R: HttpRunner<Response = Response>> CommentMergeRequest for Github<R> {
    fn create(&self, args: CommentMergeRequestBodyArgs) -> Result<()> {
        let url = format!(
            "{}/repos/{}/issues/{}/comments",
            self.rest_api_basepath, self.path, args.id
        );
        let mut body = Body::new();
        body.add("body", args.comment);
        query::create_merge_request_comment(
            &self.runner,
            &url,
            Some(body),
            self.request_headers(),
            POST,
            ApiOperation::MergeRequest,
        )?;
        Ok(())
    }

    fn list(&self, args: CommentMergeRequestListBodyArgs) -> Result<Vec<Comment>> {
        todo!()
    }
}

pub struct GithubMergeRequestFields {
    id: i64,
    web_url: String,
    source_branch: String,
    author: String,
    updated_at: String,
    created_at: String,
    title: String,
    pull_request: String,
    description: String,
    merged_at: String,
    pipeline_id: Option<i64>,
    pipeline_url: Option<String>,
}

impl From<&serde_json::Value> for GithubMergeRequestFields {
    fn from(merge_request_data: &serde_json::Value) -> Self {
        GithubMergeRequestFields {
            id: merge_request_data["number"].as_i64().unwrap(),
            web_url: merge_request_data["html_url"].as_str().unwrap().to_string(),
            source_branch: merge_request_data["head"]["ref"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            author: merge_request_data["user"]["login"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            updated_at: merge_request_data["updated_at"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            created_at: merge_request_data["created_at"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            title: merge_request_data["title"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            pull_request: merge_request_data["pull_request"]["html_url"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            description: merge_request_data["body"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            merged_at: merge_request_data["merged_at"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            // Not available in the response. Set it to the same ID as the pull request
            pipeline_id: Some(merge_request_data["number"].as_i64().unwrap()),
            pipeline_url: merge_request_data["html_url"]
                .as_str()
                .map(|url| format!("{}/checks", url)),
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
            .created_at(fields.created_at)
            .title(fields.title)
            .pull_request(fields.pull_request)
            .description(fields.description)
            .merged_at(fields.merged_at)
            .pipeline_id(fields.pipeline_id)
            .pipeline_url(fields.pipeline_url)
            .build()
            .unwrap()
    }
}

#[cfg(test)]
mod test {

    use std::sync::Arc;

    use crate::{
        http::{self, Headers},
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
    fn test_open_merge_request_on_target_repository() {
        let config = config();
        let mr_args = MergeRequestBodyArgs::builder()
            .target_repo("jordilin/gitar".to_string())
            .build()
            .unwrap();
        let domain = "github.com".to_string();
        // current repo, targetting jordilin/gitar
        let path = "jdoe/gitar";
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
            "https://api.github.com/repos/jordilin/gitar/pulls",
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
            "https://api.github.com/repos/jordilin/githapi/pulls?head=jordilin:feature",
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
    fn test_open_merge_request_cannot_get_owner_org_namespace_in_existing_pull_request() {
        let config = config();
        let mr_args = MergeRequestBodyArgs::builder()
            .source_branch("feature".to_string())
            .build()
            .unwrap();

        let domain = "github.com".to_string();
        // missing the repo name
        let path = "jordilin";
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
                Some(error::GRError::ApplicationError(_)) => (),
                _ => panic!("Expected error::GRError::ApplicationError"),
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
            .assignee_id(None)
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
            .assignee_id(None)
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

    #[test]
    fn test_get_pull_requests_for_auth_user() {
        let config = config();
        let domain = "github.com".to_string();
        let path = "jordilin/githapi";
        let response = Response::builder()
            .status(200)
            .body(get_contract(ContractType::Github, "list_issues_user.json"))
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let github: Box<dyn MergeRequest> =
            Box::new(Github::new(config, &domain, &path, client.clone()));
        let args = MergeRequestListBodyArgs::builder()
            .state(MergeRequestState::Opened)
            .list_args(None)
            .assignee_id(Some(123456))
            .build()
            .unwrap();
        let merge_requests = github.list(args).unwrap();
        assert_eq!("https://api.github.com/issues?state=open", *client.url());
        assert!(merge_requests.len() == 1);
        assert_eq!(
            Some(ApiOperation::MergeRequest),
            *client.api_operation.borrow()
        );
    }

    #[test]
    fn test_create_merge_request_comment() {
        let config = config();
        let domain = "github.com".to_string();
        let path = "jordilin/githapi";
        let response = Response::builder().status(201).build().unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let github: Box<dyn CommentMergeRequest> =
            Box::new(Github::new(config, &domain, &path, client.clone()));
        let args = CommentMergeRequestBodyArgs::builder()
            .id(23)
            .comment("Looks good to me".to_string())
            .build()
            .unwrap();
        github.create(args).unwrap();
        assert_eq!(
            "https://api.github.com/repos/jordilin/githapi/issues/23/comments",
            *client.url(),
        );
        assert_eq!(
            Some(ApiOperation::MergeRequest),
            *client.api_operation.borrow()
        );
    }

    #[test]
    fn test_create_merge_request_comment_error_status_code() {
        let config = config();
        let domain = "github.com".to_string();
        let path = "jordilin/githapi";
        let response = Response::builder().status(500).build().unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let github: Box<dyn CommentMergeRequest> =
            Box::new(Github::new(config, &domain, &path, client.clone()));
        let args = CommentMergeRequestBodyArgs::builder()
            .id(23)
            .comment("Looks good to me".to_string())
            .build()
            .unwrap();
        assert!(github.create(args).is_err());
    }

    #[test]
    fn test_close_pull_request_ok() {
        let config = config();
        let domain = "github.com".to_string();
        let path = "jordilin/githapi";
        let response = Response::builder()
            .status(200)
            .body(get_contract(ContractType::Github, "merge_request.json"))
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let github: Box<dyn MergeRequest> =
            Box::new(Github::new(config, &domain, &path, client.clone()));
        github.close(23).unwrap();
        assert_eq!(
            "https://api.github.com/repos/jordilin/githapi/pulls/23",
            *client.url(),
        );
        assert_eq!(http::Method::PATCH, *client.http_method.borrow());
        assert_eq!(
            Some(ApiOperation::MergeRequest),
            *client.api_operation.borrow()
        );
    }

    #[test]
    fn test_get_pull_request_details() {
        let config = config();
        let domain = "github.com".to_string();
        let path = "jordilin/githapi";
        let response = Response::builder()
            .status(200)
            .body(get_contract(ContractType::Github, "merge_request.json"))
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let github: Box<dyn MergeRequest> =
            Box::new(Github::new(config, &domain, &path, client.clone()));
        github.get(23).unwrap();
        assert_eq!(
            "https://api.github.com/repos/jordilin/githapi/pulls/23",
            *client.url(),
        );
        assert_eq!(
            Some(ApiOperation::MergeRequest),
            *client.api_operation.borrow()
        );
    }

    #[test]
    fn test_github_merge_pull_request() {
        let config = config();
        let domain = "github.com".to_string();
        let path = "jordilin/githapi";
        let response = Response::builder()
            .status(200)
            .body(get_contract(ContractType::Github, "merge_request.json"))
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let github: Box<dyn MergeRequest> =
            Box::new(Github::new(config, &domain, &path, client.clone()));
        github.merge(23).unwrap();
        assert_eq!(
            "https://api.github.com/repos/jordilin/githapi/pulls/23/merge",
            *client.url(),
        );
        assert_eq!(http::Method::PUT, *client.http_method.borrow());
        assert_eq!(
            Some(ApiOperation::MergeRequest),
            *client.api_operation.borrow()
        );
    }
}
