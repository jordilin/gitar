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

    fn request_headers(&self) -> Headers {
        let mut headers = Headers::new();
        let auth_token_value = format!("bearer {}", self.api_token);
        headers.set("Authorization".to_string(), auth_token_value);
        headers.set(
            "Accept".to_string(),
            "application/vnd.github.v3+json".to_string(),
        );
        headers.set("User-Agent".to_string(), "gitar".to_string());
        headers.set("X-GitHub-Api-Version".to_string(), "2022-11-28".to_string());
        headers
    }
}
