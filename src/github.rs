use serde::Serialize;

use crate::api_traits::MergeRequest;
use crate::api_traits::Remote;
use crate::api_traits::RemoteProject;
use crate::cli::BrowseOptions;
use crate::config::ConfigProperties;
use crate::error;
use crate::http;
use crate::http::Method::{GET, PATCH, POST, PUT};
use crate::http::Paginator;
use crate::io::CmdInfo;
use crate::io::HttpRunner;
use crate::io::Response;
use crate::remote::Member;
use crate::remote::MergeRequestArgs;
use crate::remote::MergeRequestResponse;
use crate::remote::MergeRequestState;
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

    fn http_request<T>(&self, url: &str, body: Option<T>, method: http::Method) -> http::Request<T>
    where
        T: Serialize,
    {
        match method {
            http::Method::GET => {
                let mut request = http::Request::new(url, http::Method::GET);
                let headers = self.request_headers();
                request.set_headers(headers);
                request
            }
            http::Method::POST => {
                let mut request =
                    http::Request::new(url, http::Method::POST).with_body(body.unwrap());
                let headers = self.request_headers();
                request.set_headers(headers);
                request
            }
            http::Method::PUT => {
                let mut request = if let Some(body) = body {
                    http::Request::new(url, http::Method::PUT).with_body(body)
                } else {
                    http::Request::new(url, http::Method::PUT)
                };
                let headers = self.request_headers();
                request.set_headers(headers);
                request
            }
            http::Method::PATCH => {
                let mut request =
                    http::Request::new(url, http::Method::PATCH).with_body(body.unwrap());
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
    fn get_project_data(&self) -> Result<CmdInfo> {
        let url = format!("{}/repos/{}", self.rest_api_basepath, self.path);
        let mut request: http::Request<()> = self.http_request(&url, None, GET);
        let response = self.runner.run(&mut request)?;
        if response.status != 200 {
            return Err(error::gen(format!(
                "Failed to get project data from Github: {}",
                response.body
            )));
        }
        let project_data: serde_json::Value = serde_json::from_str(&response.body)?;
        let project_id = project_data["id"].as_i64().unwrap();
        let default_branch = project_data["default_branch"]
            .to_string()
            .trim_matches('"')
            .to_string();
        let project = Project::new(project_id, &default_branch);
        Ok(CmdInfo::Project(project))
    }

    fn get_project_members(&self) -> Result<CmdInfo> {
        let url = &format!(
            "{}/repos/{}/contributors",
            self.rest_api_basepath, self.path
        );
        let request: http::Request<()> = self.http_request(url, None, GET);
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
                let mut members = Vec::new();
                let members_data: Vec<serde_json::Value> = serde_json::from_str(&response.body)?;
                for member_data in members_data {
                    let id = member_data["id"].as_i64().unwrap();
                    let username = member_data["login"].as_str().unwrap();
                    let name = "";
                    let member = Member::new(id, name, username);
                    members.push(member);
                }
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
    fn open(&self, args: MergeRequestArgs) -> Result<MergeRequestResponse> {
        let mut body: HashMap<&str, String> = HashMap::new();
        body.insert("head", args.source_branch().to_string());
        body.insert("base", args.target_branch().to_string());
        body.insert("title", args.title().to_string());
        body.insert("body", args.description().to_string());
        let mr_url = format!("{}/repos/{}/pulls", self.rest_api_basepath, self.path);
        let mut request = self.http_request(&mr_url, Some(body), POST);
        match self.runner.run(&mut request) {
            Ok(response) => {
                // 422 - pull request already exists.
                if response.status != 201 && response.status != 422 {
                    return Err(error::gen(format!(
                        "Failed to create merge request. Status code: {}, Body: {}",
                        response.status, response.body
                    )));
                }
                // If the pull request already exists, we need to pull its URL
                // by filtering by user:ref or org:ref
                // Response example is:
                // {
                //     "documentation_url": "https://docs.github.com/rest/reference/pulls#create-a-pull-request",
                //     "errors": [
                //       {
                //         "code": "custom",
                //         "message": "A pull request already exists for jordilin:githubmr.",
                //         "resource": "PullRequest"
                //       }
                //     ],
                //     "message": "Validation Failed"
                //   }
                let body = response.body;
                let fields = body
                    .split("A pull request already exists")
                    .collect::<Vec<&str>>();
                if fields.len() == 1 {
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
                    let merge_request_json: serde_json::Value = serde_json::from_str(&body)?;
                    let id = merge_request_json["number"].to_string();
                    let issues_url = format!(
                        "{}/repos/{}/issues/{}",
                        self.rest_api_basepath, self.path, id
                    );
                    let mut body: HashMap<&str, &Vec<&str>> = HashMap::new();
                    let assignees = vec![args.username()];
                    body.insert("assignees", &assignees);
                    self.runner
                        .run(&mut self.http_request(&issues_url, Some(body), PATCH))?;
                    return Ok(MergeRequestResponse::new(
                        merge_request_json["id"].as_i64().unwrap(),
                        merge_request_json["html_url"].to_string().trim_matches('"'),
                        "",
                        "",
                        "",
                    ));
                }
                // There is an existing pull request already.
                // Gather its URL by querying Github pull requests filtering by
                // namespace:branch
                let remote_pr_branch = format!("{}:{}", self.path, args.source_branch());
                let existing_mr_url = format!("{}?head={}", mr_url, remote_pr_branch);
                let mut request: http::Request<()> = self.http_request(&existing_mr_url, None, GET);
                // let client = http::Client::new(NoCache);
                let response = self.runner.run(&mut request)?;
                let merge_requests_json: Vec<serde_json::Value> =
                    serde_json::from_str(&response.body)?;
                if merge_requests_json.len() == 1 {
                    return Ok(MergeRequestResponse::new(
                        merge_requests_json[0]["id"].as_i64().unwrap(),
                        merge_requests_json[0]["html_url"]
                            .to_string()
                            .trim_matches('"'),
                        "",
                        "",
                        "",
                    ));
                }
                Err(error::gen("Could not retrieve current pull request url"))
            }
            Err(err) => Err(err),
        }
    }

    fn list(&self, state: MergeRequestState) -> Result<Vec<MergeRequestResponse>> {
        // TODO add sort
        let url = match state {
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
        let request: http::Request<()> = self.http_request(&url, None, GET);
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
                let mut mergerequests = Vec::new();
                let mergerequests_data: Vec<serde_json::Value> =
                    serde_json::from_str(&response.body)?;
                for mr_data in mergerequests_data {
                    let id = mr_data["number"].as_i64().unwrap();
                    let url = mr_data["html_url"].as_str().unwrap();
                    let username = mr_data["user"]["login"].as_str().unwrap();
                    let updated_at = mr_data["updated_at"].as_str().unwrap();
                    let source_branch = mr_data["head"]["ref"].as_str().unwrap();
                    mergerequests.push(MergeRequestResponse::new(
                        id,
                        url,
                        username,
                        updated_at,
                        source_branch,
                    ))
                }
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
        let mut request: http::Request<()> = self.http_request(&url, None, PUT);
        let response = self.runner.run(&mut request)?;
        if response.status != 200 {
            return Err(error::gen(format!(
                "Failed to merge merge request: {}",
                response.body
            )));
        }
        serde_json::from_str::<serde_json::Value>(&response.body)?;
        // Response:
        // {
        //     "sha": "6dcb09b5b57875f334f61aebed695e2e4193db5e",
        //     "merged": true,
        //     "message": "Pull Request successfully merged"
        // }

        // We do not have the id nor the url available in the response. Compute
        // it and return it to the client so we can open the url if needed.

        Ok(MergeRequestResponse::new(
            id,
            &self.get_url(BrowseOptions::MergeRequestId(id)),
            "",
            "",
            "",
        ))
    }

    fn get(&self, id: i64) -> Result<MergeRequestResponse> {
        let url = format!(
            "{}/repos/{}/pulls/{}",
            self.rest_api_basepath, self.path, id
        );
        let mut request: http::Request<()> = self.http_request(&url, None, GET);
        let response = self.runner.run(&mut request)?;
        let merge_request_json: serde_json::Value = serde_json::from_str(&response.body)?;
        Ok(MergeRequestResponse::new(
            merge_request_json["id"].as_i64().unwrap(),
            merge_request_json["html_url"].to_string().trim_matches('"'),
            "",
            "",
            merge_request_json["head"]["ref"]
                .to_string()
                .trim_matches('"'),
        ))
    }

    fn close(&self, _id: i64) -> Result<MergeRequestResponse> {
        todo!()
    }
}

impl<R: HttpRunner<Response = Response> + Send + Sync + 'static> Remote for Github<R> {}

#[cfg(test)]
mod test {
    use crate::test::utils::{config, get_contract, ContractType, MockRunner};

    use super::*;

    #[test]
    fn test_open_merge_request() {
        let config = config();
        let mr_args = MergeRequestArgs::new();

        let domain = "github.com".to_string();
        let path = "jordilin/githapi";
        let response1 = Response::new()
            .with_status(201)
            .with_body(get_contract(ContractType::Github, "merge_request.json"));
        let response2 = Response::new()
            .with_status(200)
            .with_body(get_contract(ContractType::Github, "merge_request.json"));
        let client = Arc::new(MockRunner::new(vec![response2, response1]));
        let github = Github::new(config, &domain, &path, client.clone());

        assert!(github.open(mr_args).is_ok());
        assert_eq!(
            "https://api.github.com/repos/jordilin/githapi/issues/1",
            *client.url(),
        );
    }

    #[test]
    fn test_open_merge_request_error_status_code() {
        let config = config();
        let mr_args = MergeRequestArgs::new();

        let domain = "github.com".to_string();
        let path = "jordilin/githapi";
        let response1 = Response::new().with_status(401).with_body(
            r#"{"message":"Bad credentials","documentation_url":"https://docs.github.com/rest"}"#
                .to_string(),
        );
        let client = Arc::new(MockRunner::new(vec![response1]));
        let github = Github::new(config, &domain, &path, client.clone());
        assert!(github.open(mr_args).is_err());
    }
}
