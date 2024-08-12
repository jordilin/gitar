use crate::config::ConfigProperties;
use crate::http::Headers;
use std::sync::Arc;
pub mod cicd;
pub mod container_registry;
pub mod gist;
pub mod merge_request;
pub mod project;
pub mod release;
pub mod trending;
pub mod user;

// https://docs.gitlab.com/ee/api/rest/

#[derive(Clone)]
pub struct Gitlab<R> {
    api_token: String,
    domain: String,
    path: String,
    projects_base_url: String,
    runner: Arc<R>,
    base_project_url: String,
    base_current_user_url: String,
    base_users_url: String,
    merge_requests_url: String,
    base_runner_url: String,
}

impl<R> Gitlab<R> {
    pub fn new(
        config: Arc<dyn ConfigProperties>,
        domain: &str,
        path: &str,
        runner: Arc<R>,
    ) -> Self {
        let api_token = config.api_token().to_string();
        let domain = domain.to_string();
        let encoded_path = encode_path(path);
        let api_path = "api/v4";
        let protocol = "https";
        let base_api_path = format!("{}://{}/{}", protocol, domain, api_path);
        let base_user_url = format!("{}/user", base_api_path);
        let base_users_url = format!("{}/users", base_api_path);
        let base_runner_url = format!("{}/runners", base_api_path);
        let merge_requests_url = format!("{}/merge_requests", base_api_path);
        let base_project_url = format!("{}/projects", base_api_path);
        let projects_base_url = format!("{}/{}", base_project_url, encoded_path);
        Gitlab {
            api_token,
            domain,
            path: path.to_string(),
            projects_base_url,
            runner,
            base_project_url,
            base_current_user_url: base_user_url,
            merge_requests_url,
            base_runner_url,
            base_users_url,
        }
    }

    fn api_token(&self) -> &str {
        &self.api_token
    }

    fn rest_api_basepath(&self) -> &str {
        &self.projects_base_url
    }

    fn headers(&self) -> Headers {
        let mut headers = Headers::new();
        headers.set("PRIVATE-TOKEN", self.api_token());
        headers
    }
}

fn encode_path(path: &str) -> String {
    path.replace('/', "%2F")
}
