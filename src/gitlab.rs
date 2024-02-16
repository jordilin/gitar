use crate::api_traits::{ApiOperation, Cicd, MergeRequest, RemoteProject};
use crate::cli::BrowseOptions;
use crate::config::ConfigProperties;
use crate::error;
use crate::http::{self, Headers, Paginator, Request};
use crate::io::Response;
use crate::io::{CmdInfo, HttpRunner};
use crate::remote::{
    query, Member, MergeRequestBodyArgs, MergeRequestListBodyArgs, MergeRequestResponse, Pipeline,
    PipelineBodyArgs, Project,
};
use crate::{json_load_page, json_loads, Result};
use std::collections::HashMap;
use std::sync::Arc;

// https://docs.gitlab.com/ee/api/rest/

#[derive(Clone)]
pub struct Gitlab<R> {
    api_token: String,
    domain: String,
    path: String,
    rest_api_basepath: String,
    runner: Arc<R>,
    base_project_url: String,
}

impl<R> Gitlab<R> {
    pub fn new(config: impl ConfigProperties, domain: &str, path: &str, runner: Arc<R>) -> Self {
        let api_token = config.api_token().to_string();
        let domain = domain.to_string();
        let encoded_path = path.replace('/', "%2F");
        let base_project_url = format!("https://{}/api/v4/projects", domain);
        let rest_api_basepath = format!("{}/{}", base_project_url, encoded_path);

        Gitlab {
            api_token,
            domain,
            path: path.to_string(),
            rest_api_basepath,
            runner,
            base_project_url,
        }
    }

    fn api_token(&self) -> &str {
        &self.api_token
    }

    fn rest_api_basepath(&self) -> &str {
        &self.rest_api_basepath
    }

    fn headers(&self) -> Headers {
        let mut headers = Headers::new();
        headers.set("PRIVATE-TOKEN", self.api_token());
        headers
    }
}

impl<R: HttpRunner<Response = Response>> MergeRequest for Gitlab<R> {
    fn open(&self, args: MergeRequestBodyArgs) -> Result<MergeRequestResponse> {
        let mut body = HashMap::new();
        body.insert("source_branch", args.source_branch);
        body.insert("target_branch", args.target_branch);
        body.insert("title", args.title);
        body.insert("assignee_id", args.assignee_id);
        body.insert("description", args.description);
        body.insert("remove_source_branch", args.remove_source_branch);
        let url = format!("{}/merge_requests", self.rest_api_basepath());
        let mut request = http::Request::new(&url, http::Method::POST)
            .with_api_operation(ApiOperation::MergeRequest)
            .with_body(body);
        request.set_header("PRIVATE-TOKEN", self.api_token());
        let response = self.runner.run(&mut request)?;
        // if status code is 409, it means that the merge request already
        // exists. We already pushed the branch, just return the merge request
        // as if it was created.
        if response.status == 409 {
            // {\"message\":[\"Another open merge request already exists for
            // this source branch: !60\"]}"
            let merge_request_json: serde_json::Value = serde_json::from_str(&response.body)?;
            let merge_request_iid = merge_request_json["message"][0]
                .as_str()
                .unwrap()
                .split_whitespace()
                .last()
                .unwrap()
                .trim_matches('!');
            let merge_request_url = format!(
                "https://{}/{}/-/merge_requests/{}",
                self.domain, self.path, merge_request_iid
            );
            return Ok(MergeRequestResponse::builder()
                .id(merge_request_iid.parse().unwrap())
                .web_url(merge_request_url)
                .build()
                .unwrap());
        }
        if response.status != 201 {
            return Err(error::gen(format!(
                "Failed to open merge request: {}",
                response.body
            )));
        }
        let merge_request_json = json_loads(&response.body)?;

        Ok(MergeRequestResponse::builder()
            .id(merge_request_json["iid"].as_i64().unwrap())
            .web_url(merge_request_json["web_url"].as_str().unwrap().to_string())
            .build()
            .unwrap())
    }

    fn list(&self, args: MergeRequestListBodyArgs) -> Result<Vec<MergeRequestResponse>> {
        let mut url = format!(
            "{}/merge_requests?state={}",
            self.rest_api_basepath(),
            args.state
        );
        let mut request: Request<()> = http::Request::new(&url, http::Method::GET)
            .with_api_operation(ApiOperation::MergeRequest);
        request.set_header("PRIVATE-TOKEN", self.api_token());
        if args.list_args.is_some() {
            let from_page = args.list_args.as_ref().unwrap().page;
            let suffix = format!("&page={}", &from_page);
            url.push_str(&suffix);
            request.set_max_pages(args.list_args.unwrap().max_pages);
            request.set_url(&url);
        }
        let paginator = Paginator::new(&self.runner, request, &url);
        paginator
            .map(|response| {
                let response = response?;
                if response.status != 200 {
                    return Err(error::gen(format!(
                        "Failed to get project merge requests from GitLab: {}",
                        response.body
                    )));
                }
                let mergerequests = json_load_page(&response.body)?.iter().fold(
                    Vec::new(),
                    |mut mergerequests, mr_data| {
                        mergerequests.push(
                            MergeRequestResponse::builder()
                                .id(mr_data["iid"].as_i64().unwrap())
                                .web_url(mr_data["web_url"].as_str().unwrap().to_string())
                                .source_branch(
                                    mr_data["source_branch"].as_str().unwrap().to_string(),
                                )
                                .author(mr_data["author"]["username"].as_str().unwrap().to_string())
                                .updated_at(mr_data["updated_at"].as_str().unwrap().to_string())
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
        // PUT /projects/:id/merge_requests/:merge_request_iid/merge
        let url = format!("{}/merge_requests/{}/merge", self.rest_api_basepath(), id);
        let merge_request_json = query::send(
            &self.runner,
            &url,
            self.headers(),
            http::Method::PUT,
            ApiOperation::MergeRequest,
        )?;
        Ok(MergeRequestResponse::builder()
            .id(merge_request_json["iid"].as_i64().unwrap())
            .web_url(merge_request_json["web_url"].as_str().unwrap().to_string())
            .build()
            .unwrap())
    }

    fn get(&self, id: i64) -> Result<MergeRequestResponse> {
        // GET /projects/:id/merge_requests/:merge_request_iid
        let url = format!("{}/merge_requests/{}", self.rest_api_basepath(), id);
        let merge_request_json = query::send(
            &self.runner,
            &url,
            self.headers(),
            http::Method::GET,
            ApiOperation::MergeRequest,
        )?;
        Ok(MergeRequestResponse::builder()
            .id(merge_request_json["iid"].as_i64().unwrap())
            .web_url(merge_request_json["web_url"].as_str().unwrap().to_string())
            .source_branch(
                merge_request_json["source_branch"]
                    .as_str()
                    .unwrap()
                    .to_string(),
            )
            .build()
            .unwrap())
    }

    fn close(&self, id: i64) -> Result<MergeRequestResponse> {
        let url = format!("{}/merge_requests/{}", self.rest_api_basepath(), id);
        let mut body = HashMap::new();
        body.insert("state_event".to_string(), "close".to_string());
        let mut request: Request<HashMap<String, String>> =
            http::Request::new(&url, http::Method::PUT)
                .with_api_operation(ApiOperation::MergeRequest)
                .with_body(body);
        request.set_header("PRIVATE-TOKEN", self.api_token());
        let response = self.runner.run(&mut request)?;
        if response.status != 200 {
            return Err(error::gen(format!(
                "Failed to close the merge request wilth URL: {} and ERROR: {}",
                url, response.body
            )));
        }
        let merge_request_json = json_loads(&response.body)?;
        Ok(MergeRequestResponse::builder()
            .id(merge_request_json["iid"].as_i64().unwrap())
            .web_url(merge_request_json["web_url"].as_str().unwrap().to_string())
            .build()
            .unwrap())
    }

    fn num_pages(&self, args: MergeRequestListBodyArgs) -> Result<Option<u32>> {
        let state = args.state.to_string();
        let url = format!(
            "{}/merge_requests?state={}&page=1",
            self.rest_api_basepath(),
            state
        );
        let mut headers = Headers::new();
        headers.set("PRIVATE-TOKEN", self.api_token());
        query::num_pages(&self.runner, &url, headers, ApiOperation::MergeRequest)
    }
}

impl<R: HttpRunner<Response = Response>> RemoteProject for Gitlab<R> {
    fn get_project_data(&self, id: Option<i64>) -> Result<CmdInfo> {
        let url = match id {
            Some(id) => format!("{}/{}", self.base_project_url, id),
            None => self.rest_api_basepath().to_string(),
        };
        let project_data = query::send(
            &self.runner,
            &url,
            self.headers(),
            http::Method::GET,
            ApiOperation::Project,
        )?;
        let project_id = project_data["id"].as_i64().unwrap();
        let default_branch = project_data["default_branch"].as_str().unwrap();
        let html_url = project_data["web_url"].as_str().unwrap();
        let project = Project::new(project_id, default_branch).with_html_url(html_url);
        Ok(CmdInfo::Project(project))
    }

    fn get_project_members(&self) -> Result<CmdInfo> {
        let url = format!("{}/members/all", self.rest_api_basepath());
        let mut request: Request<()> =
            http::Request::new(&url, http::Method::GET).with_api_operation(ApiOperation::Project);
        request.set_header("PRIVATE-TOKEN", self.api_token());
        let paginator = Paginator::new(&self.runner, request, &url);
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
                                .name(member_data["name"].as_str().unwrap().to_string())
                                .username(member_data["username"].as_str().unwrap().to_string())
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
            BrowseOptions::MergeRequests => format!("{}/merge_requests", base_url),
            BrowseOptions::MergeRequestId(id) => format!("{}/merge_requests/{}", base_url, id),
            BrowseOptions::Pipelines => format!("{}/pipelines", base_url),
        }
    }
}

impl<R: HttpRunner<Response = Response>> Cicd for Gitlab<R> {
    fn list(&self, args: PipelineBodyArgs) -> Result<Vec<Pipeline>> {
        let mut url = format!("{}/pipelines", self.rest_api_basepath());
        let mut request: Request<()> =
            http::Request::new(&url, http::Method::GET).with_api_operation(ApiOperation::Pipeline);
        request.set_header("PRIVATE-TOKEN", self.api_token());
        if args.from_to_page.is_some() {
            let from_page = args.from_to_page.as_ref().unwrap().page;
            let suffix = format!("?page={}", &from_page);
            url.push_str(&suffix);
            request.set_max_pages(args.from_to_page.unwrap().max_pages);
            request.set_url(&url);
        }
        let paginator = Paginator::new(&self.runner, request, &url);
        paginator
            .map(|response| {
                let response = response?;
                if response.status != 200 {
                    return Err(error::gen(format!(
                        "Failed to get project pipelines from GitLab: {}",
                        response.body
                    )));
                }
                let pipelines = json_load_page(&response.body)?.iter().fold(
                    Vec::new(),
                    |mut pipelines, pipeline_data| {
                        pipelines.push(
                            Pipeline::builder()
                                .status(pipeline_data["status"].as_str().unwrap().to_string())
                                .web_url(pipeline_data["web_url"].as_str().unwrap().to_string())
                                .branch(pipeline_data["ref"].as_str().unwrap().to_string())
                                .sha(pipeline_data["sha"].as_str().unwrap().to_string())
                                .created_at(
                                    pipeline_data["created_at"].as_str().unwrap().to_string(),
                                )
                                .build()
                                .unwrap(),
                        );
                        pipelines
                    },
                );
                Ok(pipelines)
            })
            .collect::<Result<Vec<Vec<Pipeline>>>>()
            .map(|pipelines| pipelines.into_iter().flatten().collect())
    }

    fn get_pipeline(&self, _id: i64) -> Result<Pipeline> {
        todo!();
    }

    fn num_pages(&self) -> Result<Option<u32>> {
        let url = format!("{}/pipelines?page=1", self.rest_api_basepath());
        let mut headers = Headers::new();
        headers.set("PRIVATE-TOKEN", self.api_token());
        query::num_pages(&self.runner, &url, headers, ApiOperation::Pipeline)
    }
}

#[cfg(test)]
mod test {

    use crate::remote::{ListBodyArgs, MergeRequestState};
    use crate::test::utils::{config, get_contract, ContractType, MockRunner};

    use crate::io::CmdInfo;

    use super::*;

    #[test]
    fn test_get_project_data_no_id() {
        let config = config();
        let domain = "gitlab.com";
        let path = "jordilin/gitlapi";
        let response = Response::builder()
            .status(200)
            .body(get_contract(ContractType::Gitlab, "project.json"))
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let gitlab = Gitlab::new(config, &domain, &path, client.clone());
        gitlab.get_project_data(None).unwrap();
        assert_eq!(
            "https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi",
            client.url().to_string(),
        );
        assert_eq!("1234", client.headers().get("PRIVATE-TOKEN").unwrap());
        assert_eq!(Some(ApiOperation::Project), *client.api_operation.borrow());
    }

    #[test]
    fn test_get_project_data_with_given_id() {
        let config = config();
        let domain = "gitlab.com";
        let path = "jordilin/gitlapi";
        let response = Response::builder()
            .status(200)
            .body(get_contract(ContractType::Gitlab, "project.json"))
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let gitlab = Gitlab::new(config, &domain, &path, client.clone());
        gitlab.get_project_data(Some(54345)).unwrap();
        assert_eq!(
            "https://gitlab.com/api/v4/projects/54345",
            client.url().to_string(),
        );
        assert_eq!("1234", client.headers().get("PRIVATE-TOKEN").unwrap());
        assert_eq!(Some(ApiOperation::Project), *client.api_operation.borrow());
    }

    #[test]
    fn test_get_project_members() {
        let config = config();
        let domain = "gitlab.com";
        let path = "jordilin/gitlapi";
        let response = Response::builder()
            .status(200)
            .body(get_contract(ContractType::Gitlab, "project_members.json"))
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let gitlab = Gitlab::new(config, &domain, &path, client.clone());

        let CmdInfo::Members(members) = gitlab.get_project_members().unwrap() else {
            panic!("Expected members");
        };
        assert_eq!(2, members.len());
        assert_eq!("test_user_0", members[0].username);
        assert_eq!("test_user_1", members[1].username);
        assert_eq!("1234", client.headers().get("PRIVATE-TOKEN").unwrap());
        assert_eq!(
            "https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/members/all",
            *client.url(),
        );
        assert_eq!(Some(ApiOperation::Project), *client.api_operation.borrow());
    }

    #[test]
    fn test_open_merge_request() {
        let config = config();

        let mr_args = MergeRequestBodyArgs::builder().build().unwrap();

        let domain = "gitlab.com".to_string();
        let path = "jordilin/gitlapi";
        let response = Response::builder()
            .status(201)
            .body(get_contract(ContractType::Gitlab, "merge_request.json"))
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let gitlab = Gitlab::new(config, &domain, &path, client.clone());

        assert!(gitlab.open(mr_args).is_ok());
        assert_eq!(
            "https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/merge_requests",
            *client.url(),
        );
        assert_eq!(
            Some(ApiOperation::MergeRequest),
            *client.api_operation.borrow()
        );
    }

    #[test]
    fn test_open_merge_request_error() {
        let config = config();

        let mr_args = MergeRequestBodyArgs::builder().build().unwrap();
        let domain = "gitlab.com".to_string();
        let path = "jordilin/gitlapi".to_string();
        let response = Response::builder().status(400).build().unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let gitlab = Gitlab::new(config, &domain, &path, client);
        assert!(gitlab.open(mr_args).is_err());
    }

    #[test]
    fn test_merge_request_already_exists_status_code_409_conflict() {
        let config = config();

        let mr_args = MergeRequestBodyArgs::builder().build().unwrap();

        let domain = "gitlab.com".to_string();
        let path = "jordilin/gitlapi".to_string();
        let response = Response::builder()
            .status(409)
            .body(get_contract(
                ContractType::Gitlab,
                "merge_request_conflict.json",
            ))
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let gitlab = Gitlab::new(config, &domain, &path, client);

        assert!(gitlab.open(mr_args).is_ok());
    }

    #[test]
    fn test_list_pipelines_ok() {
        let config = config();

        let domain = "gitlab.com".to_string();
        let path = "jordilin/gitlapi".to_string();
        let response = Response::builder()
            .status(200)
            .body(get_contract(ContractType::Gitlab, "list_pipelines.json"))
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let gitlab: Box<dyn Cicd> = Box::new(Gitlab::new(config, &domain, &path, client.clone()));
        let pipelines = gitlab.list(default_pipeline_body_args()).unwrap();

        assert_eq!(2, pipelines.len());
        assert_eq!(
            "https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/pipelines",
            *client.url(),
        );
        assert_eq!("1234", client.headers().get("PRIVATE-TOKEN").unwrap());
        assert_eq!(Some(ApiOperation::Pipeline), *client.api_operation.borrow());
    }

    fn default_pipeline_body_args() -> PipelineBodyArgs {
        let body_args = PipelineBodyArgs::builder()
            .from_to_page(None)
            .build()
            .unwrap();
        body_args
    }

    #[test]
    fn test_list_pipelines_error() {
        let config = config();

        let domain = "gitlab.com".to_string();
        let path = "jordilin/gitlapi".to_string();
        let response = Response::builder().status(400).build().unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let gitlab: Box<dyn Cicd> = Box::new(Gitlab::new(config, &domain, &path, client));

        assert!(gitlab.list(default_pipeline_body_args()).is_err());
    }

    #[test]
    fn test_no_pipelines() {
        let config = config();

        let domain = "gitlab.com".to_string();
        let path = "jordilin/gitlapi".to_string();
        let response = Response::builder()
            .status(200)
            .body(get_contract(ContractType::Gitlab, "no_pipelines.json"))
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let gitlab: Box<dyn Cicd> = Box::new(Gitlab::new(config, &domain, &path, client.clone()));
        let pipelines = gitlab.list(default_pipeline_body_args()).unwrap();
        assert_eq!(0, pipelines.len());
    }

    #[test]
    fn test_pipeline_page_from_set_in_url() {
        let config = config();
        let domain = "gitlab.com".to_string();
        let path = "jordilin/gitlapi".to_string();
        let response = Response::builder()
            .status(200)
            .body(get_contract(ContractType::Gitlab, "list_pipelines.json"))
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let gitlab: Box<dyn Cicd> = Box::new(Gitlab::new(config, &domain, &path, client.clone()));
        let fromtopage_args = ListBodyArgs::builder()
            .page(2)
            .max_pages(2)
            .build()
            .unwrap();
        let body_args = PipelineBodyArgs::builder()
            .from_to_page(Some(fromtopage_args))
            .build()
            .unwrap();
        gitlab.list(body_args).unwrap();
        assert_eq!(
            "https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/pipelines?page=2",
            *client.url(),
        );
    }

    #[test]
    fn test_gitlab_implements_num_pages_pipeline_operation() {
        let config = config();
        let domain = "gitlab.com".to_string();
        let path = "jordilin/gitlapi".to_string();
        let link_header = "<https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/pipelines?page=2>; rel=\"next\", <https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/pipelines?page=2>; rel=\"last\"";
        let headers = vec![("link".to_string(), link_header.to_string())];
        let response = Response::builder()
            .status(200)
            .headers(HashMap::from_iter(headers))
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let gitlab: Box<dyn Cicd> = Box::new(Gitlab::new(config, &domain, &path, client.clone()));
        assert_eq!(Some(2), gitlab.num_pages().unwrap());
        assert_eq!(
            "https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/pipelines?page=1",
            *client.url(),
        );
    }

    #[test]
    fn test_gitlab_num_pages_pipeline_no_last_header_in_link() {
        let config = config();
        let domain = "gitlab.com".to_string();
        let path = "jordilin/gitlapi".to_string();
        let link_header = "<https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/pipelines?page=2>; rel=\"next\"";
        let headers = vec![("link".to_string(), link_header.to_string())];
        let response = Response::builder()
            .status(200)
            .headers(HashMap::from_iter(headers))
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let gitlab: Box<dyn Cicd> = Box::new(Gitlab::new(config, &domain, &path, client.clone()));
        assert_eq!(None, gitlab.num_pages().unwrap());
    }

    #[test]
    fn test_gitlab_num_pages_pipeline_operation_response_error_is_error() {
        let config = config();
        let domain = "gitlab.com".to_string();
        let path = "jordilin/gitlapi".to_string();
        let response = Response::builder().status(400).build().unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let gitlab: Box<dyn Cicd> = Box::new(Gitlab::new(config, &domain, &path, client.clone()));
        assert!(gitlab.num_pages().is_err());
    }

    #[test]
    fn test_list_merge_request_with_from_page() {
        let config = config();
        let domain = "gitlab.com".to_string();
        let path = "jordilin/gitlapi".to_string();
        let response = Response::builder()
            .status(200)
            .body("[]".to_string())
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let gitlab: Box<dyn MergeRequest> =
            Box::new(Gitlab::new(config, &domain, &path, client.clone()));
        let args = MergeRequestListBodyArgs::builder()
            .state(MergeRequestState::Opened)
            .list_args(Some(
                ListBodyArgs::builder()
                    .page(2)
                    .max_pages(2)
                    .build()
                    .unwrap(),
            ))
            .build()
            .unwrap();
        gitlab.list(args).unwrap();
        assert_eq!(
            "https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/merge_requests?state=opened&page=2",
            *client.url(),
        );
    }

    #[test]
    fn test_gitlab_merge_request_num_pages() {
        let config = config();
        let domain = "gitlab.com".to_string();
        let path = "jordilin/gitlapi".to_string();
        let link_header = "<https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/merge_requests?state=opened&page=1>; rel=\"next\", <https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/merge_requests?state=opened&page=2>; rel=\"last\"";
        let headers = vec![("link".to_string(), link_header.to_string())];
        let response = Response::builder()
            .status(200)
            .headers(HashMap::from_iter(headers))
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let gitlab: Box<dyn MergeRequest> =
            Box::new(Gitlab::new(config, &domain, &path, client.clone()));
        let body_args = MergeRequestListBodyArgs::builder()
            .state(MergeRequestState::Opened)
            .list_args(None)
            .build()
            .unwrap();
        assert_eq!(Some(2), gitlab.num_pages(body_args).unwrap());
        assert_eq!(
            "https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/merge_requests?state=opened&page=1",
            *client.url(),
        );
    }

    #[test]
    fn test_gitlab_merge_request_num_pages_no_link_header_error() {
        let config = config();
        let domain = "gitlab.com".to_string();
        let path = "jordilin/gitlapi".to_string();
        let response = Response::builder().status(200).build().unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let gitlab: Box<dyn MergeRequest> =
            Box::new(Gitlab::new(config, &domain, &path, client.clone()));
        let body_args = MergeRequestListBodyArgs::builder()
            .state(MergeRequestState::Opened)
            .list_args(None)
            .build()
            .unwrap();
        assert!(gitlab.num_pages(body_args).is_err());
    }

    #[test]
    fn test_gitlab_merge_request_num_pages_response_error_is_error() {
        let config = config();
        let domain = "gitlab.com".to_string();
        let path = "jordilin/gitlapi".to_string();
        let response = Response::builder().status(400).build().unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let gitlab: Box<dyn MergeRequest> =
            Box::new(Gitlab::new(config, &domain, &path, client.clone()));
        let body_args = MergeRequestListBodyArgs::builder()
            .state(MergeRequestState::Opened)
            .list_args(None)
            .build()
            .unwrap();
        assert!(gitlab.num_pages(body_args).is_err());
    }

    #[test]
    fn test_gitlab_merge_request_num_pages_no_last_header_in_link() {
        let config = config();
        let domain = "gitlab.com".to_string();
        let path = "jordilin/gitlapi".to_string();
        let link_header = "<https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/merge_requests?state=opened&page=1>; rel=\"next\"";
        let headers = vec![("link".to_string(), link_header.to_string())];
        let response = Response::builder()
            .status(200)
            .headers(HashMap::from_iter(headers))
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let gitlab: Box<dyn MergeRequest> =
            Box::new(Gitlab::new(config, &domain, &path, client.clone()));
        let body_args = MergeRequestListBodyArgs::builder()
            .state(MergeRequestState::Opened)
            .list_args(None)
            .build()
            .unwrap();
        assert_eq!(None, gitlab.num_pages(body_args).unwrap());
    }
}
