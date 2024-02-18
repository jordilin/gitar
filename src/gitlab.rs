use crate::api_traits::{ApiOperation, Cicd};
use crate::config::ConfigProperties;
use crate::http::Headers;
use crate::io::HttpRunner;
use crate::io::Response;
use crate::remote::{query, Pipeline, PipelineBodyArgs};
use crate::Result;
use std::sync::Arc;
pub mod mr;
pub mod pj;

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

impl<R: HttpRunner<Response = Response>> Cicd for Gitlab<R> {
    fn list(&self, args: PipelineBodyArgs) -> Result<Vec<Pipeline>> {
        let url = format!("{}/pipelines", self.rest_api_basepath());
        query::gitlab_list_pipelines(
            &self.runner,
            &url,
            args.from_to_page,
            self.headers(),
            None,
            ApiOperation::Pipeline,
        )
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

pub struct GitlabPipelineFields {
    status: String,
    web_url: String,
    ref_: String,
    sha: String,
    created_at: String,
}

impl From<&serde_json::Value> for GitlabPipelineFields {
    fn from(data: &serde_json::Value) -> Self {
        GitlabPipelineFields {
            status: data["status"].as_str().unwrap().to_string(),
            web_url: data["web_url"].as_str().unwrap().to_string(),
            ref_: data["ref"].as_str().unwrap().to_string(),
            sha: data["sha"].as_str().unwrap().to_string(),
            created_at: data["created_at"].as_str().unwrap().to_string(),
        }
    }
}

impl From<GitlabPipelineFields> for Pipeline {
    fn from(fields: GitlabPipelineFields) -> Self {
        Pipeline::builder()
            .status(fields.status.to_string())
            .web_url(fields.web_url.to_string())
            .branch(fields.ref_.to_string())
            .sha(fields.sha.to_string())
            .created_at(fields.created_at.to_string())
            .build()
            .unwrap()
    }
}

#[cfg(test)]
mod test {

    use crate::remote::ListBodyArgs;
    use crate::test::utils::{config, get_contract, ContractType, MockRunner};

    use super::*;

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
        let mut headers = Headers::new();
        headers.set("link", link_header);
        let response = Response::builder()
            .status(200)
            .headers(headers)
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
        let mut headers = Headers::new();
        headers.set("link", link_header);
        let response = Response::builder()
            .status(200)
            .headers(headers)
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
}
