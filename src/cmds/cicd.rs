use mermaid::{generate_mermaid_stages_diagram, YamlParser};
use yaml::load_yaml;

use crate::api_traits::{Cicd, CicdRunner, Timestamp};
use crate::cli::cicd::{PipelineOptions, RunnerOptions};
use crate::config::Config;
use crate::display::{Column, DisplayBody};
use crate::remote::{GetRemoteCliArgs, ListBodyArgs, ListRemoteCliArgs};
use crate::{display, error, remote, Result};
use std::fmt::Display;
use std::io::{Read, Write};
use std::sync::Arc;

pub mod mermaid;
pub mod yaml;

use super::common::{
    self, num_cicd_pages, num_cicd_resources, num_runner_pages, num_runner_resources,
};

#[derive(Builder, Clone, Debug)]
pub struct Pipeline {
    id: i64,
    pub status: String,
    web_url: String,
    branch: String,
    sha: String,
    created_at: String,
    updated_at: String,
    duration: u64,
}

impl Pipeline {
    pub fn builder() -> PipelineBuilder {
        PipelineBuilder::default()
    }
}

impl Timestamp for Pipeline {
    fn created_at(&self) -> String {
        self.created_at.clone()
    }
}

impl From<Pipeline> for DisplayBody {
    fn from(p: Pipeline) -> DisplayBody {
        DisplayBody {
            columns: vec![
                Column::new("ID", p.id.to_string()),
                Column::new("URL", p.web_url),
                Column::new("Branch", p.branch),
                Column::new("SHA", p.sha),
                Column::new("Created at", p.created_at),
                Column::new("Updated at", p.updated_at),
                Column::new("Duration", p.duration.to_string()),
                Column::new("Status", p.status),
            ],
        }
    }
}

#[derive(Builder, Clone)]
pub struct PipelineBodyArgs {
    pub from_to_page: Option<ListBodyArgs>,
}

impl PipelineBodyArgs {
    pub fn builder() -> PipelineBodyArgsBuilder {
        PipelineBodyArgsBuilder::default()
    }
}

#[derive(Builder, Clone)]
pub struct LintFilePathArgs {
    pub path: String,
}

impl LintFilePathArgs {
    pub fn builder() -> LintFilePathArgsBuilder {
        LintFilePathArgsBuilder::default()
    }
}

#[derive(Builder, Clone)]
pub struct LintResponse {
    pub valid: bool,
    #[builder(default)]
    pub merged_yaml: String,
    pub errors: Vec<String>,
}

impl LintResponse {
    pub fn builder() -> LintResponseBuilder {
        LintResponseBuilder::default()
    }
}

pub struct YamlBytes<'a>(&'a [u8]);

impl YamlBytes<'_> {
    pub fn new(data: &[u8]) -> YamlBytes {
        YamlBytes(data)
    }
}

impl Display for YamlBytes<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = String::from_utf8_lossy(self.0);
        write!(f, "{}", s)
    }
}

#[derive(Builder, Clone)]
pub struct Runner {
    pub id: i64,
    pub active: bool,
    pub description: String,
    pub ip_address: String,
    pub name: String,
    pub online: bool,
    pub paused: bool,
    pub is_shared: bool,
    pub runner_type: String,
    pub status: String,
}

impl Runner {
    pub fn builder() -> RunnerBuilder {
        RunnerBuilder::default()
    }
}

impl From<Runner> for DisplayBody {
    fn from(r: Runner) -> DisplayBody {
        DisplayBody {
            columns: vec![
                Column::new("ID", r.id.to_string()),
                Column::new("Active", r.active.to_string()),
                Column::new("Description", r.description),
                Column::new("IP Address", r.ip_address),
                Column::new("Name", r.name),
                Column::new("Paused", r.paused.to_string()),
                Column::new("Shared", r.is_shared.to_string()),
                Column::new("Type", r.runner_type),
                Column::new("Online", r.online.to_string()),
                Column::new("Status", r.status.to_string()),
            ],
        }
    }
}

impl Timestamp for Runner {
    fn created_at(&self) -> String {
        // There is no created_at field for runners, set it to UNIX epoch
        "1970-01-01T00:00:00Z".to_string()
    }
}

/// Used when getting runner details. Adds extra fields to the runner struct.
#[derive(Builder, Clone)]
pub struct RunnerMetadata {
    pub id: i64,
    pub run_untagged: bool,
    pub tag_list: Vec<String>,
    pub version: String,
    pub architecture: String,
    pub platform: String,
    pub contacted_at: String,
    pub revision: String,
}

impl RunnerMetadata {
    pub fn builder() -> RunnerMetadataBuilder {
        RunnerMetadataBuilder::default()
    }
}

impl From<RunnerMetadata> for DisplayBody {
    fn from(r: RunnerMetadata) -> DisplayBody {
        DisplayBody {
            columns: vec![
                Column::new("ID", r.id.to_string()),
                Column::new("Run untagged", r.run_untagged.to_string()),
                Column::new("Tags", r.tag_list.join(", ")),
                Column::new("Architecture", r.architecture),
                Column::new("Platform", r.platform),
                Column::new("Contacted at", r.contacted_at),
                Column::new("Version", r.version),
                Column::new("Revision", r.revision),
            ],
        }
    }
}

#[derive(Builder, Clone)]
pub struct RunnerListCliArgs {
    pub status: RunnerStatus,
    #[builder(default)]
    pub tags: Option<String>,
    #[builder(default)]
    pub all: bool,
    pub list_args: ListRemoteCliArgs,
}

impl RunnerListCliArgs {
    pub fn builder() -> RunnerListCliArgsBuilder {
        RunnerListCliArgsBuilder::default()
    }
}

#[derive(Builder, Clone)]
pub struct RunnerListBodyArgs {
    pub list_args: Option<ListBodyArgs>,
    pub status: RunnerStatus,
    #[builder(default)]
    pub tags: Option<String>,
    #[builder(default)]
    pub all: bool,
}

impl RunnerListBodyArgs {
    pub fn builder() -> RunnerListBodyArgsBuilder {
        RunnerListBodyArgsBuilder::default()
    }
}

#[derive(Builder, Clone)]
pub struct RunnerMetadataGetCliArgs {
    pub id: i64,
    pub get_args: GetRemoteCliArgs,
}

impl RunnerMetadataGetCliArgs {
    pub fn builder() -> RunnerMetadataGetCliArgsBuilder {
        RunnerMetadataGetCliArgsBuilder::default()
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum RunnerStatus {
    Online,
    Offline,
    Stale,
    NeverContacted,
    All,
}

impl Display for RunnerStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunnerStatus::Online => write!(f, "online"),
            RunnerStatus::Offline => write!(f, "offline"),
            RunnerStatus::Stale => write!(f, "stale"),
            RunnerStatus::NeverContacted => write!(f, "never_contacted"),
            RunnerStatus::All => write!(f, "all"),
        }
    }
}

pub struct Job {
    id: i64,
    name: String,
    branch: String,
    author_name: String,
    commit_sha: String,
    pipeline_id: i64,
    runner_tags: Vec<String>,
    stage: String,
    status: String,
    created_at: String,
    started_at: String,
    finished_at: String,
    duration: u64,
}

// Technically no need to encapsulate the common ListRemoteCliArgs but we might
// need to add pipeline_id to retrieve jobs from a specific pipeline.
#[derive(Builder, Clone)]
pub struct JobListCliArgs {
    pub list_args: ListRemoteCliArgs,
}

impl JobListCliArgs {
    pub fn builder() -> JobListCliArgsBuilder {
        JobListCliArgsBuilder::default()
    }
}

pub struct JobListBodyArgs {
    list_args: Option<ListBodyArgs>,
}

pub fn execute(
    options: PipelineOptions,
    config: Arc<Config>,
    domain: String,
    path: String,
) -> Result<()> {
    match options {
        PipelineOptions::Lint(args) => {
            let remote = remote::get_cicd(domain, path, config, false)?;
            let file = std::fs::File::open(args.path)?;
            let body = read_ci_file(file)?;
            lint_ci_file(remote, &body, false, std::io::stdout())
        }
        PipelineOptions::MergedCi => {
            let remote = remote::get_cicd(domain, path, config, false)?;
            let file = std::fs::File::open(".gitlab-ci.yml")?;
            let body = read_ci_file(file)?;
            lint_ci_file(remote, &body, true, std::io::stdout())
        }
        PipelineOptions::Chart(args) => {
            let file = std::fs::File::open(".gitlab-ci.yml")?;
            let body = read_ci_file(file)?;
            let parser = YamlParser::new(load_yaml(&String::from_utf8_lossy(&body)));
            let chart = generate_mermaid_stages_diagram(parser, args)?;
            println!("{}", chart);
            Ok(())
        }
        PipelineOptions::List(cli_args) => {
            let remote = remote::get_cicd(domain, path, config, cli_args.get_args.refresh_cache)?;
            if cli_args.num_pages {
                return num_cicd_pages(remote, std::io::stdout());
            } else if cli_args.num_resources {
                return num_cicd_resources(remote, std::io::stdout());
            }
            let from_to_args = remote::validate_from_to_page(&cli_args)?;
            let body_args = PipelineBodyArgs::builder()
                .from_to_page(from_to_args)
                .build()?;
            list_pipelines(remote, body_args, cli_args, std::io::stdout())
        }
        PipelineOptions::Jobs(_) => todo!(),
        PipelineOptions::Runners(options) => match options {
            RunnerOptions::List(cli_args) => {
                let remote = remote::get_cicd_runner(
                    domain,
                    path,
                    config,
                    cli_args.list_args.get_args.refresh_cache,
                )?;
                let from_to_args = remote::validate_from_to_page(&cli_args.list_args)?;
                let tags = cli_args.tags.clone();
                let body_args = RunnerListBodyArgs::builder()
                    .list_args(from_to_args)
                    .status(cli_args.status)
                    .tags(tags)
                    .all(cli_args.all)
                    .build()?;
                if cli_args.list_args.num_pages {
                    return num_runner_pages(remote, body_args, std::io::stdout());
                }
                if cli_args.list_args.num_resources {
                    return num_runner_resources(remote, body_args, std::io::stdout());
                }
                list_runners(remote, body_args, cli_args, std::io::stdout())
            }
            RunnerOptions::Get(cli_args) => {
                let remote =
                    remote::get_cicd_runner(domain, path, config, cli_args.get_args.refresh_cache)?;
                get_runner_details(remote, cli_args, std::io::stdout())
            }
        },
    }
}

fn get_runner_details<W: Write>(
    remote: Arc<dyn CicdRunner>,
    cli_args: RunnerMetadataGetCliArgs,
    mut writer: W,
) -> Result<()> {
    let runner = remote.get(cli_args.id)?;
    display::print(&mut writer, vec![runner], cli_args.get_args)?;
    Ok(())
}

fn list_runners<W: Write>(
    remote: Arc<dyn CicdRunner>,
    body_args: RunnerListBodyArgs,
    cli_args: RunnerListCliArgs,
    mut writer: W,
) -> Result<()> {
    common::list_runners(remote, body_args, cli_args, &mut writer)
}

fn list_pipelines<W: Write>(
    remote: Arc<dyn Cicd>,
    body_args: PipelineBodyArgs,
    cli_args: ListRemoteCliArgs,
    mut writer: W,
) -> Result<()> {
    common::list_pipelines(remote, body_args, cli_args, &mut writer)
}

fn read_ci_file<R: Read>(mut reader: R) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    reader.read_to_end(&mut buf)?;
    Ok(buf)
}

fn lint_ci_file<W: Write>(
    remote: Arc<dyn Cicd>,
    body: &[u8],
    display_merged_ci_yaml: bool,
    mut writer: W,
) -> Result<()> {
    let response = remote.lint(YamlBytes::new(body))?;
    if response.valid {
        if display_merged_ci_yaml {
            let lines = response.merged_yaml.split('\n');
            for line in lines {
                if line.is_empty() {
                    continue;
                }
                writeln!(writer, "{}", line)?;
            }
            return Ok(());
        }
        writeln!(writer, "File is valid.")?;
    } else {
        for error in response.errors {
            writeln!(writer, "{}", error)?;
        }
        return Err(error::gen("Linting failed."));
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use std::io::Cursor;

    use super::*;
    use crate::error;

    #[derive(Clone, Builder)]
    struct PipelineMock {
        #[builder(default = "vec![]")]
        pipelines: Vec<Pipeline>,
        #[builder(default = "false")]
        error: bool,
        #[builder(setter(into, strip_option), default)]
        num_pages: Option<u32>,
        #[builder(default)]
        gitlab_ci_merged_yaml: String,
    }

    impl PipelineMock {
        pub fn builder() -> PipelineMockBuilder {
            PipelineMockBuilder::default()
        }
    }

    impl Cicd for PipelineMock {
        fn list(&self, _args: PipelineBodyArgs) -> Result<Vec<Pipeline>> {
            if self.error {
                return Err(error::gen("Error"));
            }
            let pp = self.pipelines.clone();
            Ok(pp)
        }

        fn get_pipeline(&self, _id: i64) -> Result<Pipeline> {
            let pp = self.pipelines.clone();
            Ok(pp[0].clone())
        }

        fn num_pages(&self) -> Result<Option<u32>> {
            if self.error {
                return Err(error::gen("Error"));
            }
            return Ok(self.num_pages);
        }

        fn num_resources(&self) -> Result<Option<crate::api_traits::NumberDeltaErr>> {
            todo!()
        }

        fn lint(&self, _body: YamlBytes) -> Result<LintResponse> {
            if self.error {
                return Ok(LintResponse::builder()
                    .valid(false)
                    .errors(vec!["YAML Error".to_string()])
                    .build()
                    .unwrap());
            }
            Ok(LintResponse::builder()
                .valid(true)
                .errors(vec![])
                .merged_yaml(self.gitlab_ci_merged_yaml.clone())
                .build()
                .unwrap())
        }
    }

    #[test]
    fn test_list_pipelines() {
        let pp_remote = PipelineMock::builder()
            .pipelines(vec![
                Pipeline::builder()
                    .id(123)
                    .status("success".to_string())
                    .web_url("https://gitlab.com/owner/repo/-/pipelines/123".to_string())
                    .branch("master".to_string())
                    .sha("1234567890abcdef".to_string())
                    .created_at("2020-01-01T00:00:00Z".to_string())
                    .updated_at("2020-01-01T00:01:00Z".to_string())
                    .duration(60)
                    .build()
                    .unwrap(),
                Pipeline::builder()
                    .id(456)
                    .status("failed".to_string())
                    .web_url("https://gitlab.com/owner/repo/-/pipelines/456".to_string())
                    .branch("master".to_string())
                    .sha("1234567890abcdef".to_string())
                    .created_at("2020-01-01T00:00:00Z".to_string())
                    .updated_at("2020-01-01T00:01:01Z".to_string())
                    .duration(61)
                    .build()
                    .unwrap(),
            ])
            .build()
            .unwrap();
        let mut buf = Vec::new();
        let body_args = PipelineBodyArgs::builder()
            .from_to_page(None)
            .build()
            .unwrap();
        let cli_args = ListRemoteCliArgs::builder().build().unwrap();
        list_pipelines(Arc::new(pp_remote), body_args, cli_args, &mut buf).unwrap();
        assert_eq!(
            String::from_utf8(buf).unwrap(),
            "ID|URL|Branch|SHA|Created at|Updated at|Duration|Status\n\
             123|https://gitlab.com/owner/repo/-/pipelines/123|master|1234567890abcdef|2020-01-01T00:00:00Z|2020-01-01T00:01:00Z|60|success\n\
             456|https://gitlab.com/owner/repo/-/pipelines/456|master|1234567890abcdef|2020-01-01T00:00:00Z|2020-01-01T00:01:01Z|61|failed\n")
    }

    #[test]
    fn test_list_pipelines_empty_warns_message() {
        let pp_remote = PipelineMock::builder().build().unwrap();
        let mut buf = Vec::new();

        let body_args = PipelineBodyArgs::builder()
            .from_to_page(None)
            .build()
            .unwrap();
        let cli_args = ListRemoteCliArgs::builder().build().unwrap();
        list_pipelines(Arc::new(pp_remote), body_args, cli_args, &mut buf).unwrap();
        assert_eq!("No resources found.\n", String::from_utf8(buf).unwrap(),)
    }

    #[test]
    fn test_pipelines_empty_with_flush_option_no_warn_message() {
        let pp_remote = PipelineMock::builder().build().unwrap();
        let mut buf = Vec::new();
        let body_args = PipelineBodyArgs::builder()
            .from_to_page(None)
            .build()
            .unwrap();
        let cli_args = ListRemoteCliArgs::builder().flush(true).build().unwrap();
        list_pipelines(Arc::new(pp_remote), body_args, cli_args, &mut buf).unwrap();
        assert_eq!("", String::from_utf8(buf).unwrap(),)
    }

    #[test]
    fn test_list_pipelines_error() {
        let pp_remote = PipelineMock::builder().error(true).build().unwrap();
        let mut buf = Vec::new();
        let body_args = PipelineBodyArgs::builder()
            .from_to_page(None)
            .build()
            .unwrap();
        let cli_args = ListRemoteCliArgs::builder().build().unwrap();
        assert!(list_pipelines(Arc::new(pp_remote), body_args, cli_args, &mut buf).is_err());
    }

    #[test]
    fn test_list_number_of_pipelines_pages() {
        let pp_remote = PipelineMock::builder().num_pages(3 as u32).build().unwrap();
        let mut buf = Vec::new();
        num_cicd_pages(Arc::new(pp_remote), &mut buf).unwrap();
        assert_eq!("3\n", String::from_utf8(buf).unwrap(),)
    }

    #[test]
    fn test_no_pages_available() {
        let pp_remote = PipelineMock::builder().build().unwrap();
        let mut buf = Vec::new();
        num_cicd_pages(Arc::new(pp_remote), &mut buf).unwrap();
        assert_eq!(
            "Number of pages not available.\n",
            String::from_utf8(buf).unwrap(),
        )
    }

    #[test]
    fn test_number_of_pages_error() {
        let pp_remote = PipelineMock::builder().error(true).build().unwrap();
        let mut buf = Vec::new();
        assert!(num_cicd_pages(Arc::new(pp_remote), &mut buf).is_err());
    }

    #[test]
    fn test_list_pipelines_no_headers() {
        let pp_remote = PipelineMock::builder()
            .pipelines(vec![
                Pipeline::builder()
                    .id(123)
                    .status("success".to_string())
                    .web_url("https://gitlab.com/owner/repo/-/pipelines/123".to_string())
                    .branch("master".to_string())
                    .sha("1234567890abcdef".to_string())
                    .created_at("2020-01-01T00:00:00Z".to_string())
                    .updated_at("2020-01-01T00:01:00Z".to_string())
                    .duration(60)
                    .build()
                    .unwrap(),
                Pipeline::builder()
                    .id(456)
                    .status("failed".to_string())
                    .web_url("https://gitlab.com/owner/repo/-/pipelines/456".to_string())
                    .branch("master".to_string())
                    .sha("1234567890abcdef".to_string())
                    .created_at("2020-01-01T00:00:00Z".to_string())
                    .updated_at("2020-01-01T00:01:00Z".to_string())
                    .duration(60)
                    .build()
                    .unwrap(),
            ])
            .build()
            .unwrap();
        let mut buf = Vec::new();
        let body_args = PipelineBodyArgs::builder()
            .from_to_page(None)
            .build()
            .unwrap();
        let cli_args = ListRemoteCliArgs::builder()
            .get_args(
                GetRemoteCliArgs::builder()
                    .no_headers(true)
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        list_pipelines(Arc::new(pp_remote), body_args, cli_args, &mut buf).unwrap();
        assert_eq!(
            "123|https://gitlab.com/owner/repo/-/pipelines/123|master|1234567890abcdef|2020-01-01T00:00:00Z|2020-01-01T00:01:00Z|60|success\n\
             456|https://gitlab.com/owner/repo/-/pipelines/456|master|1234567890abcdef|2020-01-01T00:00:00Z|2020-01-01T00:01:00Z|60|failed\n",
            String::from_utf8(buf).unwrap(),
        )
    }

    #[derive(Builder, Clone)]
    struct RunnerMock {
        #[builder(default = "vec![]")]
        runners: Vec<Runner>,
        #[builder(default)]
        error: bool,
        #[builder(default)]
        one_runner: Option<RunnerMetadata>,
    }

    impl RunnerMock {
        pub fn builder() -> RunnerMockBuilder {
            RunnerMockBuilder::default()
        }
    }

    impl CicdRunner for RunnerMock {
        fn list(&self, _args: RunnerListBodyArgs) -> Result<Vec<Runner>> {
            if self.error {
                return Err(error::gen("Error"));
            }
            let rr = self.runners.clone();
            Ok(rr)
        }

        fn get(&self, _id: i64) -> Result<RunnerMetadata> {
            let rr = self.one_runner.as_ref().unwrap();
            Ok(rr.clone())
        }

        fn num_pages(&self, _args: RunnerListBodyArgs) -> Result<Option<u32>> {
            if self.error {
                return Err(error::gen("Error"));
            }
            Ok(None)
        }

        fn num_resources(
            &self,
            _args: RunnerListBodyArgs,
        ) -> Result<Option<crate::api_traits::NumberDeltaErr>> {
            todo!()
        }
    }

    #[test]
    fn test_list_runners() {
        let runners = vec![
            Runner::builder()
                .id(1)
                .active(true)
                .description("Runner 1".to_string())
                .ip_address("10.0.0.1".to_string())
                .name("runner1".to_string())
                .online(true)
                .status("online".to_string())
                .paused(false)
                .is_shared(true)
                .runner_type("shared".to_string())
                .build()
                .unwrap(),
            Runner::builder()
                .id(2)
                .active(true)
                .description("Runner 2".to_string())
                .ip_address("10.0.0.2".to_string())
                .name("runner2".to_string())
                .online(true)
                .status("online".to_string())
                .paused(false)
                .is_shared(true)
                .runner_type("shared".to_string())
                .build()
                .unwrap(),
        ];
        let remote = RunnerMock::builder().runners(runners).build().unwrap();
        let mut buf = Vec::new();
        let body_args = RunnerListBodyArgs::builder()
            .list_args(None)
            .status(RunnerStatus::Online)
            .build()
            .unwrap();
        let cli_args = RunnerListCliArgs::builder()
            .status(RunnerStatus::Online)
            .list_args(ListRemoteCliArgs::builder().build().unwrap())
            .build()
            .unwrap();
        list_runners(Arc::new(remote), body_args, cli_args, &mut buf).unwrap();
        assert_eq!(
            "ID|Active|Description|IP Address|Name|Paused|Shared|Type|Online|Status\n\
             1|true|Runner 1|10.0.0.1|runner1|false|true|shared|true|online\n\
             2|true|Runner 2|10.0.0.2|runner2|false|true|shared|true|online\n",
            String::from_utf8(buf).unwrap()
        )
    }

    #[test]
    fn test_no_runners_warn_user_with_message() {
        let remote = RunnerMock::builder().build().unwrap();
        let mut buf = Vec::new();
        let body_args = RunnerListBodyArgs::builder()
            .list_args(None)
            .status(RunnerStatus::Online)
            .build()
            .unwrap();
        let cli_args = RunnerListCliArgs::builder()
            .status(RunnerStatus::Online)
            .list_args(ListRemoteCliArgs::builder().build().unwrap())
            .build()
            .unwrap();
        list_runners(Arc::new(remote), body_args, cli_args, &mut buf).unwrap();
        assert_eq!("No resources found.\n", String::from_utf8(buf).unwrap())
    }

    #[test]
    fn test_no_runners_found_with_flush_option_no_warn_message() {
        let remote = RunnerMock::builder().build().unwrap();
        let mut buf = Vec::new();
        let body_args = RunnerListBodyArgs::builder()
            .list_args(None)
            .status(RunnerStatus::Online)
            .build()
            .unwrap();
        let cli_args = RunnerListCliArgs::builder()
            .status(RunnerStatus::Online)
            .list_args(ListRemoteCliArgs::builder().flush(true).build().unwrap())
            .build()
            .unwrap();
        list_runners(Arc::new(remote), body_args, cli_args, &mut buf).unwrap();
        assert_eq!("", String::from_utf8(buf).unwrap())
    }

    #[test]
    fn test_get_gitlab_runner_metadata() {
        let runner_metadata = RunnerMetadata::builder()
            .id(1)
            .run_untagged(true)
            .tag_list(vec!["tag1".to_string(), "tag2".to_string()])
            .version("13.0.0".to_string())
            .architecture("amd64".to_string())
            .platform("linux".to_string())
            .contacted_at("2020-01-01T00:00:00Z".to_string())
            .revision("1234567890abcdef".to_string())
            .build()
            .unwrap();
        let remote = RunnerMock::builder()
            .one_runner(Some(runner_metadata))
            .build()
            .unwrap();
        let mut buf = Vec::new();
        let cli_args = RunnerMetadataGetCliArgs::builder()
            .id(1)
            .get_args(GetRemoteCliArgs::builder().build().unwrap())
            .build()
            .unwrap();
        get_runner_details(Arc::new(remote), cli_args, &mut buf).unwrap();
        assert_eq!(
            "ID|Run untagged|Tags|Architecture|Platform|Contacted at|Version|Revision\n\
             1|true|tag1, tag2|amd64|linux|2020-01-01T00:00:00Z|13.0.0|1234567890abcdef\n",
            String::from_utf8(buf).unwrap()
        )
    }

    fn gen_gitlab_ci_body() -> Vec<u8> {
        b"image: alpine\n\
          stages:\n\
            - build\n\
            - test\n\
          build:\n\
            stage: build\n\
            script:\n\
              - echo \"Building\"\n\
          test:\n\
            stage: test\n\
            script:\n\
              - echo \"Testing\"\n"
            .to_vec()
    }

    #[test]
    fn test_read_gitlab_ci_file_contents() {
        let expected_body = gen_gitlab_ci_body();
        let buf = Cursor::new(&expected_body);
        let body = read_ci_file(buf).unwrap();
        assert_eq!(*expected_body, *body);
    }

    #[test]
    fn test_lint_ci_file_success() {
        let mock_cicd = Arc::new(PipelineMock::builder().build().unwrap());
        let mut writer = Vec::new();
        let result = lint_ci_file(mock_cicd, &gen_gitlab_ci_body(), false, &mut writer);
        assert!(result.is_ok());
        assert_eq!(String::from_utf8(writer).unwrap(), "File is valid.\n");
    }

    #[test]
    fn test_lint_ci_file_has_errors_prints_errors() {
        let mock_cicd = Arc::new(PipelineMock::builder().error(true).build().unwrap());
        let mut writer = Vec::new();
        let result = lint_ci_file(mock_cicd, &gen_gitlab_ci_body(), false, &mut writer);
        assert!(result.is_err());
        assert_eq!(String::from_utf8(writer).unwrap(), "YAML Error\n");
    }

    #[test]
    fn test_get_merged_yaml_from_lint_response() {
        let response = LintResponse::builder()
            .valid(true)
            .merged_yaml("image: alpine\nstages:\n  - build\n  - test\nbuild:\n  stage: build\n  script:\n  - echo \"Building\"\ntest:\n  stage: test\n  script:\n  - echo \"Testing\"\n".to_string())
            .errors(vec![])
            .build()
            .unwrap();
        let mut writer = Vec::new();
        let mock_cicd = Arc::new(
            PipelineMock::builder()
                .gitlab_ci_merged_yaml(response.merged_yaml)
                .build()
                .unwrap(),
        );

        let result = lint_ci_file(mock_cicd, &gen_gitlab_ci_body(), true, &mut writer);
        assert!(result.is_ok());
        let merged_gitlab_ci = r#"image: alpine
stages:
  - build
  - test
build:
  stage: build
  script:
  - echo "Building"
test:
  stage: test
  script:
  - echo "Testing"
"#;
        assert_eq!(merged_gitlab_ci, String::from_utf8(writer).unwrap());
    }
}
