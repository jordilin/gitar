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
use crate::http::Method::{GET, PATCH, POST, PUT};
use crate::http::Paginator;
use crate::io::CmdInfo;
use crate::io::HttpRunner;
use crate::io::Response;
use crate::json_load_page;
use crate::json_loads;
use crate::remote::Member;
use crate::remote::MergeRequestBodyArgs;
use crate::remote::MergeRequestListBodyArgs;
use crate::remote::MergeRequestResponse;
use crate::remote::MergeRequestState;
use crate::remote::Pipeline;
use crate::remote::PipelineBodyArgs;
use crate::remote::Project;
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

    fn request_headers(&self) -> HashMap<String, String> {
        let mut headers = HashMap::new();
        let auth_token_value = format!("bearer {}", self.api_token);
        headers.insert("Authorization".to_string(), auth_token_value);
        headers.insert(
            "Accept".to_string(),
            "application/vnd.github.v3+json".to_string(),
        );
        headers.insert("User-Agent".to_string(), "gg".to_string());
        headers
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
        let mut request: http::Request<()> =
            self.http_request(&url, None, GET, ApiOperation::Project);
        let response = self.runner.run(&mut request)?;
        if response.status != 200 {
            return Err(error::gen(format!(
                "Failed to get project data from Github: {}",
                response.body
            )));
        }
        let project_data = json_loads(&response.body)?;
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
        let request: http::Request<()> = self.http_request(url, None, GET, ApiOperation::Project);
        let paginator = Paginator::new(&self.runner, request, url);
        let members_data = paginator
            .map(|response| {
                let response = response?;
                if response.status != 200 {
                    return Err(error::gen(format!(
                        "Failed to get project members from GitLab: {}",
                        response.body
                    )));
                }
                let members = json_load_page(&response.body)?.iter().fold(
                    Vec::new(),
                    |mut members, member_data| {
                        members.push(
                            Member::builder()
                                .id(member_data["id"].as_i64().unwrap())
                                .username(member_data["login"].as_str().unwrap().to_string())
                                .name("".to_string())
                                .build()
                                .unwrap(),
                        );
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

impl<R: HttpRunner<Response = Response>> MergeRequest for Github<R> {
    fn open(&self, args: MergeRequestBodyArgs) -> Result<MergeRequestResponse> {
        let mut body: HashMap<&str, String> = HashMap::new();
        body.insert("head", args.source_branch.clone());
        body.insert("base", args.target_branch);
        body.insert("title", args.title);
        body.insert("body", args.description);
        // Add draft in payload only when requested. It seems that Github opens
        // PR in draft mode even when the draft value is false.
        if args.draft {
            body.insert("draft", args.draft.to_string());
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
        // TODO add sort
        let url = match args.state {
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
        };
        let request: http::Request<()> =
            self.http_request(&url, None, GET, ApiOperation::MergeRequest);
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
        let mut request: http::Request<()> =
            self.http_request(&url, None, PUT, ApiOperation::MergeRequest);
        let response = self.runner.run(&mut request)?;
        if response.status != 200 {
            return Err(error::gen(format!(
                "Failed to merge merge request: {}",
                response.body
            )));
        }
        json_loads(&response.body)?;
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
        let mut request: http::Request<()> =
            self.http_request(&url, None, GET, ApiOperation::MergeRequest);
        let response = self.runner.run(&mut request)?;
        let merge_request_json = json_loads(&response.body)?;
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

    fn num_pages(&self, _args: MergeRequestListBodyArgs) -> Result<Option<u32>> {
        todo!()
    }
}

impl<R: HttpRunner<Response = Response>> Cicd for Github<R> {
    fn list(&self, _args: PipelineBodyArgs) -> Result<Vec<Pipeline>> {
        let url = format!(
            "{}/repos/{}/actions/runs",
            self.rest_api_basepath, self.path
        );
        let request: http::Request<()> = self.http_request(&url, None, GET, ApiOperation::Pipeline);
        let paginator = Paginator::new(&self.runner, request, &url);
        paginator
            .map(|response| {
                let response = response?;
                if response.status != 200 {
                    // TODO extract this into common remote utility functions.
                    return Err(GRError::RemoteUnexpectedResponseContract(format!(
                        "Failed to get project pipelines from Github: \n\
                        Expected HTTP 200, but got HTTP status code: {} \n\
                        HTTP body: {}",
                        response.status, response.body
                    ))
                    .into());
                }
                let body = json_loads(&response.body)?;
                let wrkfl_runs = body["workflow_runs"].as_array().ok_or(
                    GRError::RemoteUnexpectedResponseContract(format!(
                        "Expected an array of workflow runs but got: {}",
                        response.body
                    )),
                )?;
                let pipelines =
                    wrkfl_runs
                        .iter()
                        .fold(Vec::new(), |mut pipelines, pipeline_data| {
                            pipelines.push(
                                Pipeline::builder()
                                    .status(
                                        pipeline_data["conclusion"].as_str().unwrap().to_string(),
                                    )
                                    .web_url(
                                        pipeline_data["html_url"].as_str().unwrap().to_string(),
                                    )
                                    .branch(
                                        pipeline_data["head_branch"].as_str().unwrap().to_string(),
                                    )
                                    .sha(pipeline_data["head_sha"].as_str().unwrap().to_string())
                                    .created_at(
                                        pipeline_data["created_at"].as_str().unwrap().to_string(),
                                    )
                                    .build()
                                    .unwrap(),
                            );
                            pipelines
                        });
                Ok(pipelines)
            })
            .collect::<Result<Vec<Vec<Pipeline>>>>()
            .map(|pipelines| pipelines.into_iter().flatten().collect())
    }

    fn get_pipeline(&self, _id: i64) -> Result<Pipeline> {
        todo!()
    }

    fn num_pages(&self) -> Result<Option<u32>> {
        todo!()
    }
}

#[cfg(test)]
mod test {
    use crate::test::utils::{config, get_contract, ContractType, MockRunner};

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
                Some(error::GRError::RemoteUnexpectedResponseContract(_)) => (),
                _ => panic!("Expected error::GRError::RemoteUnexpectedResponseContract"),
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
}
