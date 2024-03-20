use super::Gitlab;
use crate::api_traits::{ApiOperation, CicdRunner};
use crate::cmds::cicd::{Pipeline, PipelineBodyArgs, Runner, RunnerListBodyArgs, RunnerMetadata};
use crate::http::{self, Headers};
use crate::remote::query;
use crate::{
    api_traits::Cicd,
    io::{HttpRunner, Response},
};
use crate::{time, Result};

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

impl<R: HttpRunner<Response = Response>> CicdRunner for Gitlab<R> {
    fn list(&self, args: RunnerListBodyArgs) -> Result<Vec<crate::cmds::cicd::Runner>> {
        let url = self.list_runners_url(&args, false);
        query::gitlab_list_project_runners(
            &self.runner,
            &url,
            args.list_args,
            self.headers(),
            None,
            ApiOperation::Pipeline,
        )
    }

    fn get(&self, id: i64) -> Result<RunnerMetadata> {
        let url = format!("{}/{}", self.base_runner_url, id);
        query::gitlab_get_runner_metadata::<_, ()>(
            &self.runner,
            &url,
            None,
            self.headers(),
            http::Method::GET,
            ApiOperation::Pipeline,
        )
    }

    fn num_pages(&self, args: RunnerListBodyArgs) -> Result<Option<u32>> {
        let url = self.list_runners_url(&args, true);
        query::num_pages(&self.runner, &url, self.headers(), ApiOperation::Pipeline)
    }
}

impl<R> Gitlab<R> {
    fn list_runners_url(&self, args: &RunnerListBodyArgs, num_pages: bool) -> String {
        let mut url = format!(
            "{}/runners?status={}",
            self.rest_api_basepath(),
            args.status
        );
        if num_pages {
            url.push_str("&page=1");
        }
        if let Some(tags) = &args.tags {
            url.push_str(&format!("&tag_list={}", tags));
        }
        url
    }
}

pub struct GitlabRunnerFields {
    id: i64,
    description: String,
    ip_address: String,
    active: bool,
    paused: bool,
    is_shared: bool,
    runner_type: String,
    name: String,
    online: bool,
    status: String,
}

impl From<&serde_json::Value> for GitlabRunnerFields {
    fn from(value: &serde_json::Value) -> Self {
        Self {
            id: value["id"].as_i64().unwrap(),
            description: value["description"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            ip_address: value["ip_address"].as_str().unwrap_or_default().to_string(),
            active: value["active"].as_bool().unwrap_or_default(),
            paused: value["paused"].as_bool().unwrap_or_default(),
            is_shared: value["is_shared"].as_bool().unwrap_or_default(),
            runner_type: value["runner_type"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            name: value["name"].as_str().unwrap_or_default().to_string(),
            online: value["online"].as_bool().unwrap_or_default(),
            status: value["status"].as_str().unwrap_or_default().to_string(),
        }
    }
}

impl From<GitlabRunnerFields> for Runner {
    fn from(fields: GitlabRunnerFields) -> Self {
        Runner::builder()
            .id(fields.id)
            .description(fields.description)
            .ip_address(fields.ip_address)
            .active(fields.active)
            .paused(fields.paused)
            .is_shared(fields.is_shared)
            .runner_type(fields.runner_type)
            .name(fields.name)
            .online(fields.online)
            .status(fields.status)
            .build()
            .unwrap()
    }
}

pub struct GitlabRunnerMetadataFields {
    pub id: i64,
    pub run_untagged: bool,
    pub tag_list: Vec<String>,
    pub version: String,
    pub architecture: String,
    pub platform: String,
    pub contacted_at: String,
    pub revision: String,
}

impl From<&serde_json::Value> for GitlabRunnerMetadataFields {
    fn from(value: &serde_json::Value) -> Self {
        Self {
            id: value["id"].as_i64().unwrap(),
            run_untagged: value["run_untagged"].as_bool().unwrap(),
            tag_list: value["tag_list"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().to_string())
                .collect(),
            version: value["version"].as_str().unwrap().to_string(),
            architecture: value["architecture"].as_str().unwrap().to_string(),
            platform: value["platform"].as_str().unwrap().to_string(),
            contacted_at: value["contacted_at"].as_str().unwrap().to_string(),
            revision: value["revision"].as_str().unwrap().to_string(),
        }
    }
}

impl From<GitlabRunnerMetadataFields> for RunnerMetadata {
    fn from(fields: GitlabRunnerMetadataFields) -> Self {
        RunnerMetadata::builder()
            .id(fields.id)
            .run_untagged(fields.run_untagged)
            .tag_list(fields.tag_list)
            .version(fields.version)
            .architecture(fields.architecture)
            .platform(fields.platform)
            .contacted_at(fields.contacted_at)
            .revision(fields.revision)
            .build()
            .unwrap()
    }
}

pub struct GitlabPipelineFields {
    status: String,
    web_url: String,
    ref_: String,
    sha: String,
    created_at: String,
    updated_at: String,
}

impl From<&serde_json::Value> for GitlabPipelineFields {
    fn from(data: &serde_json::Value) -> Self {
        GitlabPipelineFields {
            status: data["status"].as_str().unwrap().to_string(),
            web_url: data["web_url"].as_str().unwrap().to_string(),
            ref_: data["ref"].as_str().unwrap().to_string(),
            sha: data["sha"].as_str().unwrap().to_string(),
            created_at: data["created_at"].as_str().unwrap().to_string(),
            updated_at: data["updated_at"].as_str().unwrap().to_string(),
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
            .updated_at(fields.updated_at.to_string())
            .duration(time::compute_duration(
                &fields.created_at,
                &fields.updated_at,
            ))
            .build()
            .unwrap()
    }
}

#[cfg(test)]
mod test {

    use std::sync::Arc;

    use crate::cmds::cicd::RunnerStatus;
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

        assert_eq!(3, pipelines.len());
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

    #[test]
    fn test_list_project_runners() {
        let config = config();
        let domain = "gitlab.com".to_string();
        let path = "jordilin/gitlapi".to_string();
        let response = Response::builder()
            .status(200)
            .body(get_contract(
                ContractType::Gitlab,
                "list_project_runners.json",
            ))
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let gitlab: Box<dyn CicdRunner> =
            Box::new(Gitlab::new(config, &domain, &path, client.clone()));
        let body_args = RunnerListBodyArgs::builder()
            .status(RunnerStatus::Online)
            .list_args(None)
            .build()
            .unwrap();
        gitlab.list(body_args).unwrap();
        assert_eq!(
            "https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/runners?status=online",
            *client.url(),
        );
        assert_eq!("1234", client.headers().get("PRIVATE-TOKEN").unwrap());
        assert_eq!(Some(ApiOperation::Pipeline), *client.api_operation.borrow());
    }

    #[test]
    fn test_project_runner_num_pages() {
        let config = config();
        let domain = "gitlab.com".to_string();
        let path = "jordilin/gitlapi".to_string();
        let link_header = "<https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/runners?status=online&page=1>; rel=\"first\", <https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/runners?status=online&page=1>; rel=\"last\"";
        let mut headers = Headers::new();
        headers.set("link", link_header);
        let response = Response::builder()
            .status(200)
            .headers(headers)
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let gitlab: Box<dyn CicdRunner> =
            Box::new(Gitlab::new(config, &domain, &path, client.clone()));
        let body_args = RunnerListBodyArgs::builder()
            .status(RunnerStatus::Online)
            .list_args(None)
            .build()
            .unwrap();
        let num_pages = gitlab.num_pages(body_args).unwrap();
        assert_eq!(
            "https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/runners?status=online&page=1",
            *client.url(),
        );
        assert_eq!(Some(1), num_pages);
    }

    #[test]
    fn test_get_gitlab_runner_metadata() {
        let config = config();
        let domain = "gitlab.com".to_string();
        let path = "jordilin/gitlapi".to_string();
        let response = Response::builder()
            .status(200)
            .body(get_contract(
                ContractType::Gitlab,
                "get_runner_details.json",
            ))
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let gitlab: Box<dyn CicdRunner> =
            Box::new(Gitlab::new(config, &domain, &path, client.clone()));
        gitlab.get(11573930).unwrap();
        assert_eq!("https://gitlab.com/api/v4/runners/11573930", *client.url(),);
        assert_eq!("1234", client.headers().get("PRIVATE-TOKEN").unwrap());
        assert_eq!(Some(ApiOperation::Pipeline), *client.api_operation.borrow());
    }

    #[test]
    fn test_list_gitlab_runners_with_a_tag_list() {
        let config = config();
        let domain = "gitlab.com".to_string();
        let path = "jordilin/gitlapi".to_string();
        let response = Response::builder()
            .status(200)
            .body(get_contract(
                ContractType::Gitlab,
                "list_project_runners.json",
            ))
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let gitlab: Box<dyn CicdRunner> =
            Box::new(Gitlab::new(config, &domain, &path, client.clone()));
        let body_args = RunnerListBodyArgs::builder()
            .status(RunnerStatus::Online)
            .list_args(None)
            .tags(Some("tag1,tag2".to_string()))
            .build()
            .unwrap();
        gitlab.list(body_args).unwrap();
        assert_eq!(
            "https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/runners?status=online&tag_list=tag1,tag2",
            *client.url(),
        );
        assert_eq!("1234", client.headers().get("PRIVATE-TOKEN").unwrap());
        assert_eq!(Some(ApiOperation::Pipeline), *client.api_operation.borrow());
    }
}
