use serde::Serialize;

use crate::api_traits::ApiOperation;
use crate::api_traits::Cicd;
use crate::api_traits::MergeRequest;
use crate::api_traits::RemoteProject;
use crate::cli::BrowseOptions;
use crate::config::ConfigProperties;
use crate::error;
use crate::error::GRError;
use crate::http;
use crate::http::Body;
use crate::http::Headers;
use crate::http::Method::{GET, PATCH, POST, PUT};
use crate::http::Paginator;
use crate::io::CmdInfo;
use crate::io::HttpRunner;
use crate::io::Response;
use crate::json_load_page;
use crate::json_loads;
use crate::remote::query::github_list_members;
use crate::remote::query::github_list_pipelines;
use crate::remote::{
    query, Member, MergeRequestBodyArgs, MergeRequestListBodyArgs, MergeRequestResponse,
    MergeRequestState, Pipeline, PipelineBodyArgs, Project,
};
use crate::Result;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone)]
pub struct Github<R> {
    api_token: String,
    domain: String,
    path: String,
    rest_api_basepath: String,
    runner: Arc<R>,
}

impl<R> Github<R> {
    pub fn new(config: impl ConfigProperties, domain: &str, path: &str, runner: Arc<R>) -> Self {
        let api_token = config.api_token().to_string();
        let domain = domain.to_string();
        let rest_api_basepath = format!("https://api.{}", domain);

        Github {
            api_token,
            domain,
            path: path.to_string(),
            rest_api_basepath,
            runner,
        }
    }

    fn http_request<T>(
        &self,
        url: &str,
        body: Option<T>,
        method: http::Method,
        api_operation: ApiOperation,
    ) -> http::Request<T>
    where
        T: Serialize,
    {
        match method {
            http::Method::GET => {
                let mut request =
                    http::Request::new(url, http::Method::GET).with_api_operation(api_operation);
                let headers = self.request_headers();
                request.set_headers(headers);
                request
            }
            http::Method::POST => {
                let mut request = http::Request::new(url, http::Method::POST)
                    .with_api_operation(api_operation)
                    .with_body(body.unwrap());
                let headers = self.request_headers();
                request.set_headers(headers);
                request
            }
            http::Method::PUT => {
                let mut request = if let Some(body) = body {
                    http::Request::new(url, http::Method::PUT)
                        .with_api_operation(api_operation)
                        .with_body(body)
                } else {
                    http::Request::new(url, http::Method::PUT).with_api_operation(api_operation)
                };
                let headers = self.request_headers();
                request.set_headers(headers);
                request
            }
            http::Method::PATCH => {
                let mut request = http::Request::new(url, http::Method::PATCH)
                    .with_api_operation(api_operation)
                    .with_body(body.unwrap());
                let headers = self.request_headers();
                request.set_headers(headers);
                request
            }
        }
    }

    fn request_headers(&self) -> Headers {
        let mut headers = Headers::new();
        let auth_token_value = format!("bearer {}", self.api_token);
        headers.set("Authorization".to_string(), auth_token_value);
        headers.set(
            "Accept".to_string(),
            "application/vnd.github.v3+json".to_string(),
        );
        headers.set("User-Agent".to_string(), "gg".to_string());
        headers
    }

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

impl<R: HttpRunner<Response = Response>> RemoteProject for Github<R> {
    fn get_project_data(&self, id: Option<i64>) -> Result<CmdInfo> {
        if let Some(id) = id {
            return Err(GRError::OperationNotSupported(format!(
                "Getting project data by id is not supported in Github: {}",
                id
            ))
            .into());
        };
        let url = format!("{}/repos/{}", self.rest_api_basepath, self.path);
        let project_data = query::send(
            &self.runner,
            &url,
            self.request_headers(),
            GET,
            ApiOperation::Project,
        )?;
        let project_id = project_data["id"].as_i64().unwrap();
        let default_branch = project_data["default_branch"]
            .to_string()
            .trim_matches('"')
            .to_string();
        let html_url = project_data["html_url"]
            .to_string()
            .trim_matches('"')
            .to_string();
        let project = Project::new(project_id, &default_branch).with_html_url(&html_url);
        Ok(CmdInfo::Project(project))
    }

    fn get_project_members(&self) -> Result<CmdInfo> {
        let url = &format!(
            "{}/repos/{}/contributors",
            self.rest_api_basepath, self.path
        );
        let members = github_list_members(
            &self.runner,
            url,
            None,
            self.request_headers(),
            None,
            ApiOperation::Project,
        )?;
        Ok(CmdInfo::Members(members))
    }

    fn get_url(&self, option: BrowseOptions) -> String {
        let base_url = format!("https://{}/{}", self.domain, self.path);
        match option {
            BrowseOptions::Repo => base_url,
            BrowseOptions::MergeRequests => format!("{}/pulls", base_url),
            BrowseOptions::MergeRequestId(id) => format!("{}/pull/{}", base_url, id),
            BrowseOptions::Pipelines => format!("{}/actions", base_url),
        }
    }
}

pub struct GithubMemberFields {
    id: i64,
    login: String,
    name: String,
}

impl From<&serde_json::Value> for GithubMemberFields {
    fn from(member_data: &serde_json::Value) -> Self {
        GithubMemberFields {
            id: member_data["id"].as_i64().unwrap(),
            login: member_data["login"].as_str().unwrap().to_string(),
            name: "".to_string(),
        }
    }
}

impl From<GithubMemberFields> for Member {
    fn from(fields: GithubMemberFields) -> Self {
        Member::builder()
            .id(fields.id)
            .username(fields.login)
            .name(fields.name)
            .build()
            .unwrap()
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
        let mut request = self.http_request(&mr_url, Some(body), POST, ApiOperation::MergeRequest);
        match self.runner.run(&mut request) {
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
                        let mut body: HashMap<&str, &Vec<&str>> = HashMap::new();
                        let assignees = vec![args.username.as_str()];
                        body.insert("assignees", &assignees);
                        self.runner.run(&mut self.http_request(
                            &issues_url,
                            Some(body),
                            PATCH,
                            ApiOperation::MergeRequest,
                        ))?;
                        Ok(MergeRequestResponse::builder()
                            .id(merge_request_json["id"].as_i64().unwrap())
                            .web_url(
                                merge_request_json["html_url"]
                                    .to_string()
                                    .trim_matches('"')
                                    .to_string(),
                            )
                            .build()
                            .unwrap())
                    }
                    422 => {
                        // There is an existing pull request already.
                        // Gather its URL by querying Github pull requests filtering by
                        // namespace:branch
                        let remote_pr_branch = format!("{}:{}", self.path, args.source_branch);
                        let existing_mr_url = format!("{}?head={}", mr_url, remote_pr_branch);
                        let mut request: http::Request<()> = self.http_request(
                            &existing_mr_url,
                            None,
                            GET,
                            ApiOperation::MergeRequest,
                        );
                        let response = self.runner.run(&mut request)?;
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
        let mut request: http::Request<()> =
            self.http_request(&url, None, GET, ApiOperation::MergeRequest);
        if args.list_args.is_some() {
            let list_args = args.list_args.as_ref().unwrap();
            let from_page = list_args.page;
            let suffix = format!("&page={}", &from_page);
            url.push_str(&suffix);
            request.set_max_pages(list_args.max_pages);
            request.set_url(&url);
        }
        let paginator = Paginator::new(&self.runner, request, &url);
        paginator
            .map(|response| {
                let response = response?;
                if response.status != 200 {
                    return Err(error::gen(format!(
                        "Failed to get project merge requests from Github: {}",
                        response.body
                    )));
                }
                let mergerequests = json_load_page(&response.body)?.iter().fold(
                    Vec::new(),
                    |mut mergerequests, mr_data| {
                        mergerequests.push(
                            MergeRequestResponse::builder()
                                .id(mr_data["number"].as_i64().unwrap())
                                .web_url(mr_data["html_url"].as_str().unwrap().to_string())
                                .author(mr_data["user"]["login"].as_str().unwrap().to_string())
                                .updated_at(mr_data["updated_at"].as_str().unwrap().to_string())
                                .source_branch(mr_data["head"]["ref"].as_str().unwrap().to_string())
                                .build()
                                .unwrap(),
                        );
                        mergerequests
                    },
                );
                Ok(mergerequests)
            })
            .collect::<Result<Vec<Vec<MergeRequestResponse>>>>()
            .map(|mergerequests| mergerequests.into_iter().flatten().collect())
    }

    fn merge(&self, id: i64) -> Result<MergeRequestResponse> {
        // https://docs.github.com/en/rest/pulls/pulls?apiVersion=2022-11-28#merge-a-pull-request
        //  /repos/{owner}/{repo}/pulls/{pull_number}/merge
        let url = format!(
            "{}/repos/{}/pulls/{}/merge",
            self.rest_api_basepath, self.path, id
        );
        query::send(
            &self.runner,
            &url,
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
        let merge_request_json = query::send(
            &self.runner,
            &url,
            self.request_headers(),
            GET,
            ApiOperation::MergeRequest,
        )?;
        Ok(MergeRequestResponse::builder()
            .id(merge_request_json["id"].as_i64().unwrap())
            .web_url(
                merge_request_json["html_url"]
                    .to_string()
                    .trim_matches('"')
                    .to_string(),
            )
            .source_branch(
                merge_request_json["head"]["ref"]
                    .to_string()
                    .trim_matches('"')
                    .to_string(),
            )
            .build()
            .unwrap())
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

impl<R: HttpRunner<Response = Response>> Cicd for Github<R> {
    fn list(&self, args: PipelineBodyArgs) -> Result<Vec<Pipeline>> {
        // Doc:
        // https://docs.github.com/en/rest/actions/workflow-runs?apiVersion=2022-11-28#list-workflow-runs-for-a-repository
        let url = format!(
            "{}/repos/{}/actions/runs",
            self.rest_api_basepath, self.path
        );
        github_list_pipelines(
            &self.runner,
            &url,
            args.from_to_page,
            self.request_headers(),
            Some("workflow_runs"),
            ApiOperation::Pipeline,
        )
    }

    fn get_pipeline(&self, _id: i64) -> Result<Pipeline> {
        todo!()
    }

    fn num_pages(&self) -> Result<Option<u32>> {
        let url = format!(
            "{}/repos/{}/actions/runs?page=1",
            self.rest_api_basepath, self.path
        );
        let headers = self.request_headers();
        query::num_pages(&self.runner, &url, headers, ApiOperation::Pipeline)
    }
}

pub struct GithubPipelineFields {
    status: String,
    web_url: String,
    branch: String,
    sha: String,
    created_at: String,
}

impl From<&serde_json::Value> for GithubPipelineFields {
    fn from(pipeline_data: &serde_json::Value) -> Self {
        GithubPipelineFields {
            // Github has `conclusion` as the final
            // state of the pipeline. It also has a
            // `status` field to represent the current
            // state of the pipeline. Our domain
            // `Pipeline` struct `status` refers to the
            // final state, i.e conclusion.
            status: pipeline_data["conclusion"]
                .as_str()
                // conclusion is not present when a
                // pipeline is running, gather its status.
                .unwrap_or_else(|| {
                    pipeline_data["status"]
                        .as_str()
                        // set is as unknown if
                        // neither conclusion nor
                        // status are present.
                        .unwrap_or("unknown")
                })
                .to_string(),
            web_url: pipeline_data["html_url"].as_str().unwrap().to_string(),
            branch: pipeline_data["head_branch"].as_str().unwrap().to_string(),
            sha: pipeline_data["head_sha"].as_str().unwrap().to_string(),
            created_at: pipeline_data["created_at"].as_str().unwrap().to_string(),
        }
    }
}

impl From<GithubPipelineFields> for Pipeline {
    fn from(fields: GithubPipelineFields) -> Self {
        Pipeline::builder()
            .status(fields.status)
            .web_url(fields.web_url)
            .branch(fields.branch)
            .sha(fields.sha)
            .created_at(fields.created_at)
            .build()
            .unwrap()
    }
}

#[cfg(test)]
mod test {
    use crate::{
        remote::ListBodyArgs,
        test::utils::{config, get_contract, ContractType, MockRunner},
    };

    use super::*;

    #[test]
    fn test_get_project_data_no_id() {
        let config = config();
        let domain = "github.com".to_string();
        let path = "jordilin/githapi";
        let response = Response::builder()
            .status(200)
            .body(get_contract(ContractType::Github, "project.json"))
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let github = Github::new(config, &domain, &path, client.clone());
        github.get_project_data(None).unwrap();
        assert_eq!(
            "https://api.github.com/repos/jordilin/githapi",
            *client.url(),
        );
        assert_eq!(Some(ApiOperation::Project), *client.api_operation.borrow());
    }

    #[test]
    fn test_get_project_data_with_id_not_supported() {
        let config = config();
        let domain = "github.com".to_string();
        let path = "jordilin/githapi";
        let client = Arc::new(MockRunner::new(vec![]));
        let github = Github::new(config, &domain, &path, client.clone());
        assert!(github.get_project_data(Some(1)).is_err());
    }

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
    fn test_list_actions() {
        let config = config();
        let domain = "github.com".to_string();
        let path = "jordilin/githapi";
        let response = Response::builder()
            .status(200)
            .body(get_contract(ContractType::Github, "list_pipelines.json"))
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let github: Box<dyn Cicd> = Box::new(Github::new(config, &domain, &path, client.clone()));
        let args = PipelineBodyArgs::builder()
            .from_to_page(None)
            .build()
            .unwrap();
        let runs = github.list(args).unwrap();
        assert_eq!(
            "https://api.github.com/repos/jordilin/githapi/actions/runs",
            *client.url(),
        );
        assert_eq!(Some(ApiOperation::Pipeline), *client.api_operation.borrow());
        assert_eq!(1, runs.len());
    }

    #[test]
    fn test_list_actions_error_status_code() {
        let config = config();
        let domain = "github.com".to_string();
        let path = "jordilin/githapi";
        let response = Response::builder().status(401).build().unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let github: Box<dyn Cicd> = Box::new(Github::new(config, &domain, &path, client.clone()));
        let args = PipelineBodyArgs::builder()
            .from_to_page(None)
            .build()
            .unwrap();
        assert!(github.list(args).is_err());
    }

    #[test]
    fn test_list_actions_unexpected_ok_status_code() {
        let config = config();
        let domain = "github.com".to_string();
        let path = "jordilin/githapi";
        let response = Response::builder().status(302).build().unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let github: Box<dyn Cicd> = Box::new(Github::new(config, &domain, &path, client.clone()));
        let args = PipelineBodyArgs::builder()
            .from_to_page(None)
            .build()
            .unwrap();
        match github.list(args) {
            Ok(_) => panic!("Expected error"),
            Err(err) => match err.downcast_ref::<error::GRError>() {
                Some(error::GRError::RemoteServerError(_)) => (),
                _ => panic!("Expected error::GRError::RemoteServerError"),
            },
        }
    }

    #[test]
    fn test_list_actions_empty_workflow_runs() {
        let config = config();
        let domain = "github.com".to_string();
        let path = "jordilin/githapi";
        let response = Response::builder()
            .status(200)
            .body(r#"{"workflow_runs":[]}"#.to_string())
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let github: Box<dyn Cicd> = Box::new(Github::new(config, &domain, &path, client.clone()));
        let args = PipelineBodyArgs::builder()
            .from_to_page(None)
            .build()
            .unwrap();
        assert_eq!(0, github.list(args).unwrap().len());
    }

    #[test]
    fn test_workflow_runs_not_an_array_is_error() {
        let config = config();
        let domain = "github.com".to_string();
        let path = "jordilin/githapi";
        let response = Response::builder()
            .status(200)
            .body(r#"{"workflow_runs":{}}"#.to_string())
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let github: Box<dyn Cicd> = Box::new(Github::new(config, &domain, &path, client.clone()));
        let args = PipelineBodyArgs::builder()
            .from_to_page(None)
            .build()
            .unwrap();
        match github.list(args) {
            Ok(_) => panic!("Expected error"),
            Err(err) => match err.downcast_ref::<error::GRError>() {
                Some(error::GRError::RemoteUnexpectedResponseContract(_)) => (),
                _ => panic!("Expected error::GRError::RemoteUnexpectedResponseContract"),
            },
        }
    }

    #[test]
    fn test_num_pages_for_list_actions() {
        let config = config();
        let domain = "github.com".to_string();
        let path = "jordilin/githapi";
        let link_header = r#"<https://api.github.com/repos/jordilin/githapi/actions/runs?page=1>; rel="next", <https://api.github.com/repos/jordilin/githapi/actions/runs?page=1>; rel="last""#;
        let response = Response::builder()
            .status(200)
            .headers(HashMap::from_iter(vec![(
                "link".to_string(),
                link_header.to_string(),
            )]))
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let github: Box<dyn Cicd> = Box::new(Github::new(config, &domain, &path, client.clone()));
        assert_eq!(Some(1), github.num_pages().unwrap());
        assert_eq!(
            "https://api.github.com/repos/jordilin/githapi/actions/runs?page=1",
            *client.url(),
        );
        assert_eq!(Some(ApiOperation::Pipeline), *client.api_operation.borrow());
    }

    #[test]
    fn test_num_pages_error_retrieving_last_page() {
        let config = config();
        let domain = "github.com".to_string();
        let path = "jordilin/githapi";
        let response = Response::builder().status(200).build().unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let github: Box<dyn Cicd> = Box::new(Github::new(config, &domain, &path, client.clone()));
        assert!(github.num_pages().is_err());
    }

    #[test]
    fn test_list_actions_from_page_set_in_url() {
        let config = config();
        let domain = "github.com".to_string();
        let path = "jordilin/githapi";
        let response = Response::builder()
            .status(200)
            .body(get_contract(ContractType::Github, "list_pipelines.json"))
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let github: Box<dyn Cicd> = Box::new(Github::new(config, &domain, &path, client.clone()));
        let args = PipelineBodyArgs::builder()
            .from_to_page(Some(
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
            "https://api.github.com/repos/jordilin/githapi/actions/runs?page=2",
            *client.url(),
        );
        assert_eq!(Some(ApiOperation::Pipeline), *client.api_operation.borrow());
    }

    #[test]
    fn test_list_actions_conclusion_field_not_available_use_status() {
        let config = config();
        let domain = "github.com".to_string();
        let path = "jordilin/githapi";
        let contract_json = get_contract(ContractType::Github, "list_pipelines.json");
        let contract_json = contract_json
            .lines()
            .filter(|line| !line.contains("conclusion"))
            .collect::<Vec<&str>>()
            .join("\n");
        let response = Response::builder()
            .status(200)
            .body(contract_json)
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let github: Box<dyn Cicd> = Box::new(Github::new(config, &domain, &path, client.clone()));
        let args = PipelineBodyArgs::builder()
            .from_to_page(None)
            .build()
            .unwrap();
        let runs = github.list(args).unwrap();
        assert_eq!("completed", runs[0].status);
    }

    #[test]
    fn test_list_actions_neither_conclusion_nor_status_use_unknown() {
        let config = config();
        let domain = "github.com".to_string();
        let path = "jordilin/githapi";
        let contract_json = get_contract(ContractType::Github, "list_pipelines.json");
        let contract_json = contract_json
            .lines()
            .filter(|line| !line.contains("conclusion") && !line.contains("status"))
            .collect::<Vec<&str>>()
            .join("\n");
        let response = Response::builder()
            .status(200)
            .body(contract_json)
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let github: Box<dyn Cicd> = Box::new(Github::new(config, &domain, &path, client.clone()));
        let args = PipelineBodyArgs::builder()
            .from_to_page(None)
            .build()
            .unwrap();
        let runs = github.list(args).unwrap();
        assert_eq!("unknown", runs[0].status);
    }

    #[test]
    fn test_merge_request_num_pages() {
        let config = config();
        let domain = "github.com".to_string();
        let path = "jordilin/githapi";
        let link_header = r#"<https://api.github.com/repos/jordilin/githapi/pulls?state=open&page=2>; rel="next", <https://api.github.com/repos/jordilin/githapi/pulls?state=open&page=2>; rel="last""#;
        let response = Response::builder()
            .status(200)
            .headers(HashMap::from_iter(vec![(
                "link".to_string(),
                link_header.to_string(),
            )]))
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
