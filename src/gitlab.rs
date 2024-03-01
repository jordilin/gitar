use crate::config::ConfigProperties;
use crate::http::Headers;
use std::sync::Arc;
pub mod cicd;
pub mod container_registry;
pub mod merge_request;
pub mod project;

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
