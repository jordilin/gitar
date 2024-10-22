use super::Github;
use crate::api_traits::{ApiOperation, CicdJob, CicdRunner, NumberDeltaErr};
use crate::cmds::cicd::{
    Job, JobListBodyArgs, LintResponse, Pipeline, PipelineBodyArgs, RunnerListBodyArgs,
    RunnerMetadata, RunnerPostDataCliArgs, RunnerRegistrationResponse, YamlBytes,
};
use crate::remote::query;
use crate::{
    api_traits::Cicd,
    io::{HttpResponse, HttpRunner},
};
use crate::{http, time, Result};

impl<R: HttpRunner<Response = HttpResponse>> Cicd for Github<R> {
    fn list(&self, args: PipelineBodyArgs) -> Result<Vec<Pipeline>> {
        // Doc:
        // https://docs.github.com/en/rest/actions/workflow-runs?apiVersion=2022-11-28#list-workflow-runs-for-a-repository
        let url = format!(
            "{}/repos/{}/actions/runs",
            self.rest_api_basepath, self.path
        );
        query::paged(
            &self.runner,
            &url,
            args.from_to_page,
            self.request_headers(),
            Some("workflow_runs"),
            ApiOperation::Pipeline,
            |value| GithubPipelineFields::from(value).into(),
        )
    }

    fn get_pipeline(&self, _id: i64) -> Result<Pipeline> {
        todo!()
    }

    fn num_pages(&self) -> Result<Option<u32>> {
        let (url, headers) = self.resource_cicd_metadata_url();
        query::num_pages(&self.runner, &url, headers, ApiOperation::Pipeline)
    }

    fn num_resources(&self) -> Result<Option<NumberDeltaErr>> {
        let (url, headers) = self.resource_cicd_metadata_url();
        query::num_resources(&self.runner, &url, headers, ApiOperation::Pipeline)
    }

    fn lint(&self, _body: YamlBytes) -> Result<LintResponse> {
        todo!()
    }
}

impl<R> Github<R> {
    fn resource_cicd_metadata_url(&self) -> (String, http::Headers) {
        let url = format!(
            "{}/repos/{}/actions/runs?page=1",
            self.rest_api_basepath, self.path
        );
        let headers = self.request_headers();
        (url, headers)
    }
}

impl<R: HttpRunner<Response = HttpResponse>> CicdRunner for Github<R> {
    fn list(&self, _args: RunnerListBodyArgs) -> Result<Vec<crate::cmds::cicd::Runner>> {
        todo!();
    }

    fn get(&self, _id: i64) -> Result<RunnerMetadata> {
        todo!();
    }

    fn num_pages(&self, _args: RunnerListBodyArgs) -> Result<Option<u32>> {
        todo!();
    }

    fn num_resources(&self, _args: RunnerListBodyArgs) -> Result<Option<NumberDeltaErr>> {
        todo!()
    }

    fn create(&self, _args: RunnerPostDataCliArgs) -> Result<RunnerRegistrationResponse> {
        todo!()
    }
}

impl<R: HttpRunner<Response = HttpResponse>> CicdJob for Github<R> {
    fn list(&self, _args: JobListBodyArgs) -> Result<Vec<Job>> {
        todo!();
    }

    fn num_pages(&self, _args: JobListBodyArgs) -> Result<Option<u32>> {
        todo!();
    }

    fn num_resources(
        &self,
        _args: JobListBodyArgs,
    ) -> Result<Option<crate::api_traits::NumberDeltaErr>> {
        todo!();
    }
}

pub struct GithubPipelineFields {
    pipeline: Pipeline,
}

impl From<&serde_json::Value> for GithubPipelineFields {
    fn from(pipeline_data: &serde_json::Value) -> Self {
        GithubPipelineFields {
            pipeline: Pipeline::builder()
                .id(pipeline_data["id"].as_i64().unwrap_or_default())
                // Github has `conclusion` as the final
                // state of the pipeline. It also has a
                // `status` field to represent the current
                // state of the pipeline. Our domain
                // `Pipeline` struct `status` refers to the
                // final state, i.e conclusion.
                .status(
                    pipeline_data["conclusion"]
                        .as_str()
                        // conclusion is not present when a
                        // pipeline is running, gather its status.
                        .unwrap_or_else(||
                            // set is as unknown if
// neither conclusion nor status are present.
                            pipeline_data["status"].as_str().unwrap_or("unknown"))
                        .to_string(),
                )
                .web_url(pipeline_data["html_url"].as_str().unwrap().to_string())
                .branch(pipeline_data["head_branch"].as_str().unwrap().to_string())
                .sha(pipeline_data["head_sha"].as_str().unwrap().to_string())
                .created_at(pipeline_data["created_at"].as_str().unwrap().to_string())
                .updated_at(pipeline_data["updated_at"].as_str().unwrap().to_string())
                .duration(time::compute_duration(
                    pipeline_data["created_at"].as_str().unwrap(),
                    pipeline_data["updated_at"].as_str().unwrap(),
                ))
                .build()
                .unwrap(),
        }
    }
}

impl From<GithubPipelineFields> for Pipeline {
    fn from(fields: GithubPipelineFields) -> Self {
        fields.pipeline
    }
}

#[cfg(test)]
mod test {

    use crate::{
        error,
        http::Headers,
        remote::ListBodyArgs,
        setup_client,
        test::utils::{default_github, get_contract, ContractType, ResponseContracts},
    };

    use super::*;

    #[test]
    fn test_list_actions() {
        let contracts = ResponseContracts::new(ContractType::Github).add_contract(
            200,
            "list_pipelines.json",
            None,
        );
        let (client, github) = setup_client!(contracts, default_github(), dyn Cicd);
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
        let contracts =
            ResponseContracts::new(ContractType::Github).add_body::<String>(401, None, None);
        let (_, github) = setup_client!(contracts, default_github(), dyn Cicd);
        let args = PipelineBodyArgs::builder()
            .from_to_page(None)
            .build()
            .unwrap();
        assert!(github.list(args).is_err());
    }

    #[test]
    fn test_list_actions_unexpected_ok_status_code() {
        let contracts =
            ResponseContracts::new(ContractType::Github).add_body::<String>(302, None, None);
        let (_, github) = setup_client!(contracts, default_github(), dyn Cicd);
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
        let contracts = ResponseContracts::new(ContractType::Github).add_body::<String>(
            200,
            Some(r#"{"workflow_runs":[]}"#.to_string()),
            None,
        );
        let (_, github) = setup_client!(contracts, default_github(), dyn Cicd);
        let args = PipelineBodyArgs::builder()
            .from_to_page(None)
            .build()
            .unwrap();
        assert_eq!(0, github.list(args).unwrap().len());
    }

    #[test]
    fn test_workflow_runs_not_an_array_is_error() {
        let contracts = ResponseContracts::new(ContractType::Github).add_body::<String>(
            200,
            Some(r#"{"workflow_runs":{}}"#.to_string()),
            None,
        );
        let (_, github) = setup_client!(contracts, default_github(), dyn Cicd);
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
        let link_header = r#"<https://api.github.com/repos/jordilin/githapi/actions/runs?page=1>; rel="next", <https://api.github.com/repos/jordilin/githapi/actions/runs?page=1>; rel="last""#;
        let mut headers = Headers::new();
        headers.set("link".to_string(), link_header.to_string());
        let contracts = ResponseContracts::new(ContractType::Github).add_body::<String>(
            200,
            None,
            Some(headers),
        );
        let (client, github) = setup_client!(contracts, default_github(), dyn Cicd);
        assert_eq!(Some(1), github.num_pages().unwrap());
        assert_eq!(
            "https://api.github.com/repos/jordilin/githapi/actions/runs?page=1",
            *client.url(),
        );
        assert_eq!(Some(ApiOperation::Pipeline), *client.api_operation.borrow());
    }

    #[test]
    fn test_num_pages_error_retrieving_last_page() {
        let contracts =
            ResponseContracts::new(ContractType::Github).add_body::<String>(200, None, None);
        let (_, github) = setup_client!(contracts, default_github(), dyn Cicd);
        assert_eq!(Some(1), github.num_pages().unwrap());
    }

    #[test]
    fn test_list_actions_from_page_set_in_url() {
        let contracts = ResponseContracts::new(ContractType::Github).add_contract(
            200,
            "list_pipelines.json",
            None,
        );
        let (client, github) = setup_client!(contracts, default_github(), dyn Cicd);
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
        let contract_json = get_contract(ContractType::Github, "list_pipelines.json");
        let contract_json = contract_json
            .lines()
            .filter(|line| !line.contains("conclusion"))
            .collect::<Vec<&str>>()
            .join("\n");
        let contracts = ResponseContracts::new(ContractType::Github).add_body::<String>(
            200,
            Some(contract_json),
            None,
        );
        let (_, github) = setup_client!(contracts, default_github(), dyn Cicd);
        let args = PipelineBodyArgs::builder()
            .from_to_page(None)
            .build()
            .unwrap();
        let runs = github.list(args).unwrap();
        assert_eq!("completed", runs[0].status);
    }

    #[test]
    fn test_list_actions_neither_conclusion_nor_status_use_unknown() {
        let contract_json = get_contract(ContractType::Github, "list_pipelines.json");
        let contract_json = contract_json
            .lines()
            .filter(|line| !line.contains("conclusion") && !line.contains("status"))
            .collect::<Vec<&str>>()
            .join("\n");
        let contracts = ResponseContracts::new(ContractType::Github).add_body::<String>(
            200,
            Some(contract_json),
            None,
        );
        let (_, github) = setup_client!(contracts, default_github(), dyn Cicd);
        let args = PipelineBodyArgs::builder()
            .from_to_page(None)
            .build()
            .unwrap();
        let runs = github.list(args).unwrap();
        assert_eq!("unknown", runs[0].status);
    }
}
