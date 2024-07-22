use super::Gitlab;
use crate::api_traits::{ApiOperation, CicdJob, CicdRunner};
use crate::cmds::cicd::{
    Job, JobListBodyArgs, LintResponse, Pipeline, PipelineBodyArgs, Runner, RunnerListBodyArgs,
    RunnerMetadata, RunnerPostDataCliArgs, RunnerRegistrationResponse, RunnerStatus, YamlBytes,
};
use crate::http::{self, Body, Headers};
use crate::remote::{query, URLQueryParamBuilder};
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
        let (url, headers) = self.resource_cicd_metadata_url();
        query::num_pages(&self.runner, &url, headers, ApiOperation::Pipeline)
    }

    fn num_resources(&self) -> Result<Option<crate::api_traits::NumberDeltaErr>> {
        let (url, headers) = self.resource_cicd_metadata_url();
        query::num_resources(&self.runner, &url, headers, ApiOperation::Pipeline)
    }

    // https://docs.gitlab.com/ee/api/lint.html#validate-the-ci-yaml-configuration
    fn lint(&self, body: YamlBytes) -> Result<LintResponse> {
        let url = format!("{}/ci/lint", self.rest_api_basepath());
        let mut payload = Body::new();
        payload.add("content", body.to_string());
        query::gitlab_lint_ci_file(
            &self.runner,
            &url,
            Some(&payload),
            self.headers(),
            http::Method::POST,
            ApiOperation::Pipeline,
        )
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

    fn num_resources(
        &self,
        args: RunnerListBodyArgs,
    ) -> Result<Option<crate::api_traits::NumberDeltaErr>> {
        let url = self.list_runners_url(&args, true);
        query::num_resources(&self.runner, &url, self.headers(), ApiOperation::Pipeline)
    }

    /// Creates a new runner based in the authentication token workflow as
    /// opposed to the registration based workflow which gets deprecated in
    /// Gitlab > 16.0. The response includes an auth token that can be included
    /// in the runner's configuration file.
    /// API doc https://docs.gitlab.com/ee/api/users.html#create-a-runner-linked-to-a-user
    fn create(&self, args: RunnerPostDataCliArgs) -> Result<RunnerRegistrationResponse> {
        let url = format!("{}/runners", self.base_current_user_url);
        let mut body = Body::new();
        if args.description.is_some() {
            body.add("description", args.description.unwrap());
        }
        if args.tags.is_some() {
            body.add("tag_list", args.tags.unwrap());
        }
        body.add("runner_type", args.kind.to_string());

        query::gitlab_create_runner(
            &self.runner,
            &url,
            Some(&body),
            self.headers(),
            http::Method::POST,
            ApiOperation::Pipeline,
        )
    }
}

pub struct GitlabCreateRunnerFields {
    field: RunnerRegistrationResponse,
}

impl From<&serde_json::Value> for GitlabCreateRunnerFields {
    fn from(data: &serde_json::Value) -> Self {
        GitlabCreateRunnerFields {
            field: RunnerRegistrationResponse::builder()
                .id(data["id"].as_i64().unwrap_or_default())
                .token(data["token"].as_str().unwrap_or_default().to_string())
                .token_expiration(
                    data["token_expiration"]
                        .as_str()
                        .unwrap_or_default()
                        .to_string(),
                )
                .build()
                .unwrap(),
        }
    }
}

impl From<GitlabCreateRunnerFields> for RunnerRegistrationResponse {
    fn from(fields: GitlabCreateRunnerFields) -> Self {
        fields.field
    }
}

pub struct GitlabProjectJobFields {
    job: Job,
}

impl From<&serde_json::Value> for GitlabProjectJobFields {
    fn from(data: &serde_json::Value) -> Self {
        GitlabProjectJobFields {
            job: Job::builder()
                .id(data["id"].as_i64().unwrap_or_default())
                .name(data["name"].as_str().unwrap_or_default().to_string())
                .branch(data["ref"].as_str().unwrap_or_default().to_string())
                .url(data["web_url"].as_str().unwrap_or_default().to_string())
                .author_name(
                    data["user"]["name"]
                        .as_str()
                        .unwrap_or_default()
                        .to_string(),
                )
                .commit_sha(
                    data["commit"]["id"]
                        .as_str()
                        .unwrap_or_default()
                        .to_string(),
                )
                .pipeline_id(data["pipeline"]["id"].as_i64().unwrap_or_default())
                .runner_tags(
                    data["tag_list"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|v| v.as_str().unwrap().to_string())
                        .collect(),
                )
                .stage(data["stage"].as_str().unwrap_or_default().to_string())
                .status(data["status"].as_str().unwrap_or_default().to_string())
                .created_at(data["created_at"].as_str().unwrap_or_default().to_string())
                .started_at(data["started_at"].as_str().unwrap_or_default().to_string())
                .finished_at(data["finished_at"].as_str().unwrap_or_default().to_string())
                .duration(data["duration"].as_f64().unwrap_or_default().to_string())
                .build()
                .unwrap(),
        }
    }
}

impl From<GitlabProjectJobFields> for Job {
    fn from(fields: GitlabProjectJobFields) -> Self {
        fields.job
    }
}

impl<R: HttpRunner<Response = Response>> CicdJob for Gitlab<R> {
    // https://docs.gitlab.com/ee/api/jobs.html#list-project-jobs
    fn list(&self, args: JobListBodyArgs) -> Result<Vec<Job>> {
        let url = format!("{}/jobs", self.rest_api_basepath());
        query::gitlab_list_project_jobs(
            &self.runner,
            &url,
            args.list_args,
            self.headers(),
            None,
            ApiOperation::Pipeline,
        )
    }

    fn num_pages(&self, _args: JobListBodyArgs) -> Result<Option<u32>> {
        let url = format!("{}/jobs?page=1", self.rest_api_basepath());
        query::num_pages(&self.runner, &url, self.headers(), ApiOperation::Pipeline)
    }

    fn num_resources(
        &self,
        _args: JobListBodyArgs,
    ) -> Result<Option<crate::api_traits::NumberDeltaErr>> {
        let url = format!("{}/jobs?page=1", self.rest_api_basepath());
        query::num_resources(&self.runner, &url, self.headers(), ApiOperation::Pipeline)
    }
}

impl<R> Gitlab<R> {
    fn list_runners_url(&self, args: &RunnerListBodyArgs, num_pages: bool) -> String {
        let base_url = if args.all {
            format!("{}/all", self.base_runner_url)
        } else {
            format!("{}/runners", self.rest_api_basepath(),)
        };
        let mut url = URLQueryParamBuilder::new(&base_url);
        match args.status {
            RunnerStatus::All => {}
            _ => {
                url.add_param("status", &args.status.to_string());
            }
        }
        if num_pages {
            url.add_param("page", "1");
        }
        if let Some(tags) = &args.tags {
            url.add_param("tag_list", tags);
        }
        url.build()
    }

    fn resource_cicd_metadata_url(&self) -> (String, Headers) {
        let url = format!("{}/pipelines?page=1", self.rest_api_basepath());
        let mut headers = Headers::new();
        headers.set("PRIVATE-TOKEN", self.api_token());
        (url, headers)
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
    pipeline: Pipeline,
}

impl From<&serde_json::Value> for GitlabPipelineFields {
    fn from(data: &serde_json::Value) -> Self {
        GitlabPipelineFields {
            pipeline: Pipeline::builder()
                .id(data["id"].as_i64().unwrap_or_default())
                .status(data["status"].as_str().unwrap().to_string())
                .web_url(data["web_url"].as_str().unwrap().to_string())
                .branch(data["ref"].as_str().unwrap().to_string())
                .sha(data["sha"].as_str().unwrap().to_string())
                .created_at(data["created_at"].as_str().unwrap().to_string())
                .updated_at(data["updated_at"].as_str().unwrap().to_string())
                .duration(time::compute_duration(
                    data["created_at"].as_str().unwrap(),
                    data["updated_at"].as_str().unwrap(),
                ))
                .build()
                .unwrap(),
        }
    }
}

impl From<GitlabPipelineFields> for Pipeline {
    fn from(fields: GitlabPipelineFields) -> Self {
        fields.pipeline
    }
}

pub struct GitlabLintResponseFields {
    lint_response: LintResponse,
}

impl From<&serde_json::Value> for GitlabLintResponseFields {
    fn from(data: &serde_json::Value) -> Self {
        GitlabLintResponseFields {
            lint_response: LintResponse::builder()
                .valid(data["valid"].as_bool().unwrap())
                .errors(
                    data["errors"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|v| v.as_str().unwrap().to_string())
                        .collect(),
                )
                .merged_yaml(data["merged_yaml"].as_str().unwrap().to_string())
                .build()
                .unwrap(),
        }
    }
}

impl From<GitlabLintResponseFields> for LintResponse {
    fn from(fields: GitlabLintResponseFields) -> Self {
        fields.lint_response
    }
}

#[cfg(test)]
mod test {

    use crate::cmds::cicd::{RunnerStatus, RunnerType};
    use crate::remote::ListBodyArgs;
    use crate::setup_client;
    use crate::test::utils::{default_gitlab, ContractType, ResponseContracts};

    use super::*;

    #[test]
    fn test_list_pipelines_ok() {
        let contracts = ResponseContracts::new(ContractType::Gitlab).add_contract(
            200,
            "list_pipelines.json",
            None,
        );
        let (client, gitlab) = setup_client!(contracts, default_gitlab(), dyn Cicd);
        let pipelines = gitlab.list(default_pipeline_body_args()).unwrap();

        assert_eq!(3, pipelines.len());
        assert_eq!(
            "https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/pipelines",
            *client.url(),
        );
        assert_eq!("1234", client.headers().get("PRIVATE-TOKEN").unwrap());
        assert_eq!(Some(ApiOperation::Pipeline), *client.api_operation.borrow());
    }

    #[test]
    fn test_list_pipelines_with_stream_ok() {
        let contracts = ResponseContracts::new(ContractType::Gitlab).add_contract(
            200,
            "list_pipelines.json",
            None,
        );
        let (_, gitlab) = setup_client!(contracts, default_gitlab(), dyn Cicd);
        let pipelines = gitlab
            .list(
                PipelineBodyArgs::builder()
                    .from_to_page(Some(ListBodyArgs::builder().flush(true).build().unwrap()))
                    .build()
                    .unwrap(),
            )
            .unwrap();
        // pipelines is empty because we are flushing the output to STDOUT on
        // each request
        assert_eq!(0, pipelines.len());
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
        let contracts =
            ResponseContracts::new(ContractType::Gitlab).add_body::<String>(400, None, None);
        let (_, gitlab) = setup_client!(contracts, default_gitlab(), dyn Cicd);
        assert!(gitlab.list(default_pipeline_body_args()).is_err());
    }

    #[test]
    fn test_no_pipelines() {
        let contracts = ResponseContracts::new(ContractType::Gitlab).add_contract(
            200,
            "no_pipelines.json",
            None,
        );
        let (_, gitlab) = setup_client!(contracts, default_gitlab(), dyn Cicd);
        let pipelines = gitlab.list(default_pipeline_body_args()).unwrap();
        assert_eq!(0, pipelines.len());
    }

    #[test]
    fn test_pipeline_page_from_set_in_url() {
        let contracts = ResponseContracts::new(ContractType::Gitlab).add_contract(
            200,
            "list_pipelines.json",
            None,
        );
        let (client, gitlab) = setup_client!(contracts, default_gitlab(), dyn Cicd);
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
        let link_header = "<https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/pipelines?page=2>; rel=\"next\", <https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/pipelines?page=2>; rel=\"last\"";
        let mut headers = Headers::new();
        headers.set("link", link_header);
        let contracts = ResponseContracts::new(ContractType::Gitlab).add_body::<String>(
            200,
            None,
            Some(headers),
        );
        let (client, gitlab) = setup_client!(contracts, default_gitlab(), dyn Cicd);
        assert_eq!(Some(2), gitlab.num_pages().unwrap());
        assert_eq!(
            "https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/pipelines?page=1",
            *client.url(),
        );
    }

    #[test]
    fn test_gitlab_num_pages_pipeline_no_last_header_in_link() {
        let link_header = "<https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/pipelines?page=2>; rel=\"next\"";
        let mut headers = Headers::new();
        headers.set("link", link_header);
        let contracts = ResponseContracts::new(ContractType::Gitlab).add_body::<String>(
            200,
            None,
            Some(headers),
        );
        let (_, gitlab) = setup_client!(contracts, default_gitlab(), dyn Cicd);
        assert_eq!(None, gitlab.num_pages().unwrap());
    }

    #[test]
    fn test_gitlab_num_pages_pipeline_operation_response_error_is_error() {
        let contracts =
            ResponseContracts::new(ContractType::Gitlab).add_body::<String>(400, None, None);
        let (_, gitlab) = setup_client!(contracts, default_gitlab(), dyn Cicd);
        assert!(gitlab.num_pages().is_err());
    }

    #[test]
    fn test_list_project_runners() {
        let contracts = ResponseContracts::new(ContractType::Gitlab).add_contract(
            200,
            "list_project_runners.json",
            None,
        );
        let (client, gitlab) = setup_client!(contracts, default_gitlab(), dyn CicdRunner);
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
        let link_header = "<https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/runners?status=online&page=1>; rel=\"first\", <https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/runners?status=online&page=1>; rel=\"last\"";
        let mut headers = Headers::new();
        headers.set("link", link_header);
        let contracts = ResponseContracts::new(ContractType::Gitlab).add_body::<String>(
            200,
            None,
            Some(headers),
        );
        let (client, gitlab) = setup_client!(contracts, default_gitlab(), dyn CicdRunner);
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
        let contracts = ResponseContracts::new(ContractType::Gitlab).add_contract(
            200,
            "get_runner_details.json",
            None,
        );
        let (client, gitlab) = setup_client!(contracts, default_gitlab(), dyn CicdRunner);
        gitlab.get(11573930).unwrap();
        assert_eq!("https://gitlab.com/api/v4/runners/11573930", *client.url(),);
        assert_eq!("1234", client.headers().get("PRIVATE-TOKEN").unwrap());
        assert_eq!(Some(ApiOperation::Pipeline), *client.api_operation.borrow());
    }

    #[test]
    fn test_list_gitlab_runners_with_a_tag_list() {
        let contracts = ResponseContracts::new(ContractType::Gitlab).add_contract(
            200,
            "list_project_runners.json",
            None,
        );
        let (client, gitlab) = setup_client!(contracts, default_gitlab(), dyn CicdRunner);
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

    #[test]
    fn test_get_all_gitlab_runners() {
        let contracts = ResponseContracts::new(ContractType::Gitlab).add_contract(
            200,
            // using same contract as listing project's runners. The schema is
            // the same
            "list_project_runners.json",
            None,
        );
        let (client, gitlab) = setup_client!(contracts, default_gitlab(), dyn CicdRunner);
        let body_args = RunnerListBodyArgs::builder()
            .status(RunnerStatus::Online)
            .list_args(None)
            .all(true)
            .build()
            .unwrap();
        let runners = gitlab.list(body_args).unwrap();
        assert_eq!(
            "https://gitlab.com/api/v4/runners/all?status=online",
            *client.url(),
        );
        assert_eq!("1234", client.headers().get("PRIVATE-TOKEN").unwrap());
        assert_eq!(Some(ApiOperation::Pipeline), *client.api_operation.borrow());
        assert_eq!(2, runners.len());
    }

    #[test]
    fn test_get_all_gitlab_runners_stream_ok() {
        let contracts = ResponseContracts::new(ContractType::Gitlab).add_contract(
            200,
            // using same contract as listing project's runners. The schema is
            // the same
            "list_project_runners.json",
            None,
        );
        let (client, gitlab) = setup_client!(contracts, default_gitlab(), dyn CicdRunner);
        let body_args = RunnerListBodyArgs::builder()
            .status(RunnerStatus::Online)
            .list_args(Some(ListBodyArgs::builder().flush(true).build().unwrap()))
            .all(true)
            .build()
            .unwrap();
        let runners = gitlab.list(body_args).unwrap();
        assert_eq!(
            "https://gitlab.com/api/v4/runners/all?status=online",
            *client.url(),
        );
        assert_eq!("1234", client.headers().get("PRIVATE-TOKEN").unwrap());
        assert_eq!(Some(ApiOperation::Pipeline), *client.api_operation.borrow());
        // We are streaming the output to STDOUT so we should not have any runners
        // on response.
        assert_eq!(0, runners.len());
    }

    #[test]
    fn test_get_project_runners_in_any_status() {
        let contracts = ResponseContracts::new(ContractType::Gitlab).add_contract(
            200,
            "list_project_runners.json",
            None,
        );
        let (client, gitlab) = setup_client!(contracts, default_gitlab(), dyn CicdRunner);
        let body_args = RunnerListBodyArgs::builder()
            .status(RunnerStatus::All)
            .list_args(None)
            .build()
            .unwrap();
        gitlab.list(body_args).unwrap();
        assert_eq!(
            "https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/runners",
            *client.url(),
        );
        assert_eq!("1234", client.headers().get("PRIVATE-TOKEN").unwrap());
        assert_eq!(Some(ApiOperation::Pipeline), *client.api_operation.borrow());
    }

    #[test]
    fn test_all_runners_at_any_status_with_tags() {
        let contracts = ResponseContracts::new(ContractType::Gitlab).add_contract(
            200,
            "list_project_runners.json",
            None,
        );
        let (client, gitlab) = setup_client!(contracts, default_gitlab(), dyn CicdRunner);
        let body_args = RunnerListBodyArgs::builder()
            .status(RunnerStatus::All)
            .list_args(None)
            .tags(Some("tag1,tag2".to_string()))
            .all(true)
            .build()
            .unwrap();
        gitlab.list(body_args).unwrap();
        assert_eq!(
            "https://gitlab.com/api/v4/runners/all?tag_list=tag1,tag2",
            *client.url(),
        );
        assert_eq!("1234", client.headers().get("PRIVATE-TOKEN").unwrap());
        assert_eq!(Some(ApiOperation::Pipeline), *client.api_operation.borrow());
    }

    #[test]
    fn test_all_runners_at_any_status_with_tags_num_pages() {
        let link_header = "<https://gitlab.com/api/v4/runners/all?tag_list=tag1,tag2&page=1>; rel=\"first\", <https://gitlab.com/api/v4/runners/all?tag_list=tag1,tag2&page=1>; rel=\"last\"";
        let mut headers = Headers::new();
        headers.set("link", link_header);
        let contracts = ResponseContracts::new(ContractType::Gitlab).add_body::<String>(
            200,
            None,
            Some(headers),
        );
        let (client, gitlab) = setup_client!(contracts, default_gitlab(), dyn CicdRunner);
        let body_args = RunnerListBodyArgs::builder()
            .status(RunnerStatus::All)
            .list_args(None)
            .tags(Some("tag1,tag2".to_string()))
            .all(true)
            .build()
            .unwrap();
        let num_pages = gitlab.num_pages(body_args).unwrap();
        assert_eq!(
            "https://gitlab.com/api/v4/runners/all?page=1&tag_list=tag1,tag2",
            *client.url(),
        );
        assert_eq!(Some(1), num_pages);
    }

    fn gen_gitlab_ci_body<'a>() -> YamlBytes<'a> {
        YamlBytes::new(
            b"image: alpine\n\
          stages:\n\
            - build\n\
          build:\n\
            stage: build\n\
            script:\n\
              - echo \"Building\"\n",
        )
    }

    #[test]
    fn test_lint_ci_file_ok() {
        let contracts =
            ResponseContracts::new(ContractType::Gitlab).add_contract(201, "ci_lint_ok.json", None);
        let (client, gitlab) = setup_client!(contracts, default_gitlab(), dyn Cicd);
        let response = gitlab.lint(gen_gitlab_ci_body()).unwrap();
        assert!(response.valid);
        assert_eq!(
            "https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/ci/lint",
            *client.url()
        );
    }

    #[test]
    fn test_lint_ci_file_error() {
        let contracts = ResponseContracts::new(ContractType::Gitlab).add_contract(
            201,
            "ci_lint_error.json",
            None,
        );
        let (_, gitlab) = setup_client!(contracts, default_gitlab(), dyn Cicd);
        let result = gitlab.lint(gen_gitlab_ci_body());
        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(!response.valid);
        assert!(response.errors.len() > 0);
    }

    #[test]
    fn test_gitlab_project_pipeline_jobs() {
        let contracts = ResponseContracts::new(ContractType::Gitlab).add_contract(
            200,
            "list_project_jobs.json",
            None,
        );
        let (client, gitlab) = setup_client!(contracts, default_gitlab(), dyn CicdJob);
        let body_args = JobListBodyArgs::builder().list_args(None).build().unwrap();
        gitlab.list(body_args).unwrap();
        assert_eq!(
            "https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/jobs",
            *client.url()
        );
        assert_eq!("1234", client.headers().get("PRIVATE-TOKEN").unwrap());
        assert_eq!(Some(ApiOperation::Pipeline), *client.api_operation.borrow());
    }

    #[test]
    fn test_gitlab_project_jobs_num_pages() {
        let link_header = "<https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/jobs?page=2>; rel=\"next\", <https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/jobs?page=2>; rel=\"last\"";
        let mut headers = Headers::new();
        headers.set("link", link_header);
        let contracts = ResponseContracts::new(ContractType::Gitlab).add_body::<String>(
            200,
            None,
            Some(headers),
        );
        let (client, gitlab) = setup_client!(contracts, default_gitlab(), dyn CicdJob);
        let body_args = JobListBodyArgs::builder().list_args(None).build().unwrap();
        let num_pages = gitlab.num_pages(body_args).unwrap();
        assert_eq!(
            "https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/jobs?page=1",
            *client.url(),
        );
        assert_eq!(Some(2), num_pages);
    }

    #[test]
    fn test_gitlab_project_jobs_num_resources() {
        let contracts = ResponseContracts::new(ContractType::Gitlab).add_body::<String>(
            200,
            None,
            Some(Headers::new()),
        );
        let (client, gitlab) = setup_client!(contracts, default_gitlab(), dyn CicdJob);
        let body_args = JobListBodyArgs::builder().list_args(None).build().unwrap();
        let num_resources = gitlab.num_resources(body_args).unwrap();
        assert_eq!(
            "https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/jobs?page=1",
            *client.url()
        );
        assert_eq!(Some(ApiOperation::Pipeline), *client.api_operation.borrow());
        assert_eq!("(1, 30)", &num_resources.unwrap().to_string());
    }

    #[test]
    fn test_gitlab_create_auth_token_based_instance_runner_with_description_and_tags() {
        let contracts = ResponseContracts::new(ContractType::Gitlab).add_contract(
            201,
            "create_auth_runner_response.json",
            None,
        );
        let (client, gitlab) = setup_client!(contracts, default_gitlab(), dyn CicdRunner);
        let args = RunnerPostDataCliArgs::builder()
            .description(Some("My runner".to_string()))
            .tags(Some("tag1,tag2".to_string()))
            .kind(RunnerType::Instance)
            .build()
            .unwrap();
        let response = gitlab.create(args).unwrap();
        assert_eq!("https://gitlab.com/api/v4/user/runners", *client.url(),);
        assert_eq!("1234", client.headers().get("PRIVATE-TOKEN").unwrap());
        assert_eq!(Some(ApiOperation::Pipeline), *client.api_operation.borrow());
        assert_eq!("newtoken", response.token);
        let body = client.request_body();
        assert!(body.contains("description"));
        assert!(body.contains("tag_list"));
        assert!(body.contains("instance_type"));
    }
}
