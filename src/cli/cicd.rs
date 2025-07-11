use clap::{Parser, ValueEnum};

use crate::{
    cmds::cicd::{
        mermaid::ChartType, JobListCliArgs, LintFilePathArgs, RunnerListCliArgs,
        RunnerMetadataGetCliArgs, RunnerPostDataCliArgs, RunnerStatus, RunnerType,
    },
    remote::ListRemoteCliArgs,
};

use super::common::{GetArgs, ListArgs};

#[derive(Parser)]
pub struct PipelineCommand {
    #[clap(subcommand)]
    subcommand: PipelineSubcommand,
}

#[derive(Parser)]
enum PipelineSubcommand {
    #[clap(about = "Lint ci yml files. Default is .gitlab-ci.yml")]
    Lint(FilePathArgs),
    #[clap(
        about = "Get merged .gitlab-ci.yml. Total .gitlab-ci.yml result of merging included yaml pipeline files in the repository"
    )]
    MergedCi,
    #[clap(about = "Create a Mermaid diagram of the .gitlab-ci.yml pipeline")]
    Chart(ChartArgs),
    #[clap(about = "List pipelines")]
    List(ListArgs),
    #[clap(subcommand, name = "jb", about = "Job operations")]
    Jobs(JobsSubCommand),
    #[clap(subcommand, name = "rn", about = "Runner operations")]
    Runners(RunnerSubCommand),
}

#[derive(Parser)]
enum JobsSubCommand {
    #[clap(about = "List jobs")]
    List(ListJob),
}

#[derive(Parser)]
struct ListJob {
    #[command(flatten)]
    list_args: ListArgs,
}

#[derive(Parser)]
struct FilePathArgs {
    /// Path to the ci yml file.
    #[clap(default_value = ".gitlab-ci.yml")]
    path: String,
}

#[derive(Parser)]
struct ChartArgs {
    /// Chart variant. Stages with jobs, stages or just jobs
    #[clap(long, default_value = "stageswithjobs")]
    chart_type: ChartTypeCli,
}

#[derive(ValueEnum, Clone, PartialEq, Debug)]
enum ChartTypeCli {
    #[clap(name = "stageswithjobs")]
    StagesWithJobs,
    Jobs,
    Stages,
}

#[derive(Parser)]
enum RunnerSubCommand {
    #[clap(about = "List runners")]
    List(ListRunner),
    #[clap(about = "Get runner metadata")]
    Get(RunnerMetadata),
    #[clap(about = "Create a new runner")]
    Create(RunnerPostData),
}

#[derive(ValueEnum, Clone, PartialEq, Debug)]
enum RunnerStatusCli {
    Online,
    Offline,
    Stale,
    NeverContacted,
    All,
}

#[derive(Parser)]
struct ListRunner {
    /// Runner status
    #[clap()]
    status: RunnerStatusCli,
    /// Comma separated list of tags
    #[clap(long, value_delimiter = ',', help_heading = "Runner options")]
    tags: Option<Vec<String>>,
    /// List all runners available across all projects. Gitlab admins only.
    #[clap(long, help_heading = "Runner options")]
    all: bool,
    #[command(flatten)]
    list_args: ListArgs,
}

#[derive(Parser)]
struct RunnerMetadata {
    /// Runner ID
    #[clap()]
    id: i64,
    #[clap(flatten)]
    get_args: GetArgs,
}

#[derive(Parser, Default)]
struct RunnerPostData {
    /// Runner description
    #[clap(long)]
    description: Option<String>,
    /// Runner tags. Comma separated list of tags
    #[clap(long, value_delimiter = ',')]
    tags: Option<Vec<String>>,
    /// Runner type
    #[clap(long)]
    kind: RunnerTypeCli,
    #[clap(long)]
    /// Run untagged
    run_untagged: bool,
    /// Project id. Required if runner type is project
    #[clap(long, group = "runner_target_id")]
    project_id: Option<i64>,
    /// Group id. Required if runner type is group
    #[clap(long, group = "runner_target_id")]
    group_id: Option<i64>,
}

impl RunnerPostData {
    fn validate_runner_type_id(&self) -> Result<(), String> {
        if self.kind == RunnerTypeCli::Project && self.project_id.is_none() {
            return Err("error: project id is required for project runner".to_string());
        }
        if self.kind == RunnerTypeCli::Group && self.group_id.is_none() {
            return Err("error: group id is required for group runner".to_string());
        }
        if self.kind == RunnerTypeCli::Instance
            && (self.project_id.is_some() || self.group_id.is_some())
        {
            return Err(
                "error: project id and group id are not required for instance runner".to_string(),
            );
        }
        Ok(())
    }
}

#[derive(ValueEnum, Clone, PartialEq, Debug, Default)]
enum RunnerTypeCli {
    #[default]
    Instance,
    Group,
    Project,
}

impl From<ChartTypeCli> for ChartType {
    fn from(chart_type: ChartTypeCli) -> Self {
        match chart_type {
            ChartTypeCli::StagesWithJobs => ChartType::StagesWithJobs,
            ChartTypeCli::Jobs => ChartType::Jobs,
            ChartTypeCli::Stages => ChartType::Stages,
        }
    }
}

impl From<ChartArgs> for ChartType {
    fn from(args: ChartArgs) -> Self {
        args.chart_type.into()
    }
}

impl From<ChartArgs> for PipelineOptions {
    fn from(options: ChartArgs) -> Self {
        PipelineOptions::Chart(options.into())
    }
}

impl From<PipelineCommand> for PipelineOptions {
    fn from(options: PipelineCommand) -> Self {
        match options.subcommand {
            PipelineSubcommand::Lint(options) => options.into(),
            PipelineSubcommand::MergedCi => PipelineOptions::MergedCi,
            PipelineSubcommand::Chart(options) => PipelineOptions::Chart(options.into()),
            PipelineSubcommand::List(options) => options.into(),
            PipelineSubcommand::Runners(options) => options.into(),
            PipelineSubcommand::Jobs(options) => options.into(),
        }
    }
}

impl From<FilePathArgs> for PipelineOptions {
    fn from(options: FilePathArgs) -> Self {
        PipelineOptions::Lint(options.into())
    }
}

impl From<FilePathArgs> for LintFilePathArgs {
    fn from(options: FilePathArgs) -> Self {
        LintFilePathArgs::builder()
            .path(options.path)
            .build()
            .unwrap()
    }
}

impl From<ListArgs> for PipelineOptions {
    fn from(options: ListArgs) -> Self {
        PipelineOptions::List(options.into())
    }
}

impl From<RunnerSubCommand> for PipelineOptions {
    fn from(options: RunnerSubCommand) -> Self {
        match options {
            RunnerSubCommand::List(options) => PipelineOptions::Runners(options.into()),
            RunnerSubCommand::Get(options) => PipelineOptions::Runners(options.into()),
            RunnerSubCommand::Create(options) => PipelineOptions::Runners(options.into()),
        }
    }
}

impl From<RunnerStatusCli> for RunnerStatus {
    fn from(status: RunnerStatusCli) -> Self {
        match status {
            RunnerStatusCli::Online => RunnerStatus::Online,
            RunnerStatusCli::Offline => RunnerStatus::Offline,
            RunnerStatusCli::Stale => RunnerStatus::Stale,
            RunnerStatusCli::NeverContacted => RunnerStatus::NeverContacted,
            RunnerStatusCli::All => RunnerStatus::All,
        }
    }
}

impl From<ListRunner> for RunnerOptions {
    fn from(options: ListRunner) -> Self {
        RunnerOptions::List(
            RunnerListCliArgs::builder()
                .status(options.status.into())
                .tags(options.tags.map(|tags| tags.join(",").to_string()))
                .all(options.all)
                .list_args(options.list_args.into())
                .build()
                .unwrap(),
        )
    }
}

impl From<RunnerMetadata> for RunnerOptions {
    fn from(options: RunnerMetadata) -> Self {
        RunnerOptions::Get(
            RunnerMetadataGetCliArgs::builder()
                .id(options.id)
                .get_args(options.get_args.into())
                .build()
                .unwrap(),
        )
    }
}

impl From<RunnerPostData> for RunnerOptions {
    fn from(options: RunnerPostData) -> Self {
        if let Err(e) = options.validate_runner_type_id() {
            eprintln!("{e}");
            std::process::exit(2);
        };
        RunnerOptions::Create(
            RunnerPostDataCliArgs::builder()
                .description(options.description)
                .tags(options.tags.map(|tags| tags.join(",").to_string()))
                .kind(options.kind.into())
                .run_untagged(options.run_untagged)
                .project_id(options.project_id)
                .group_id(options.group_id)
                .build()
                .unwrap(),
        )
    }
}

impl From<RunnerTypeCli> for RunnerType {
    fn from(kind: RunnerTypeCli) -> Self {
        match kind {
            RunnerTypeCli::Instance => RunnerType::Instance,
            RunnerTypeCli::Group => RunnerType::Group,
            RunnerTypeCli::Project => RunnerType::Project,
        }
    }
}

impl From<ListJob> for JobOptions {
    fn from(options: ListJob) -> Self {
        JobOptions::List(
            JobListCliArgs::builder()
                .list_args(options.list_args.into())
                .build()
                .unwrap(),
        )
    }
}

impl From<JobsSubCommand> for PipelineOptions {
    fn from(options: JobsSubCommand) -> Self {
        match options {
            JobsSubCommand::List(options) => PipelineOptions::Jobs(options.into()),
        }
    }
}

pub enum PipelineOptions {
    Lint(LintFilePathArgs),
    List(ListRemoteCliArgs),
    Runners(RunnerOptions),
    MergedCi,
    Chart(ChartType),
    Jobs(JobOptions),
}

pub enum JobOptions {
    List(JobListCliArgs),
}

pub enum RunnerOptions {
    List(RunnerListCliArgs),
    Get(RunnerMetadataGetCliArgs),
    Create(RunnerPostDataCliArgs),
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::cli::{Args, Command};

    #[test]
    fn test_pipeline_cli_list() {
        let args = Args::parse_from(vec![
            "gr",
            "pp",
            "list",
            "--from-page",
            "1",
            "--to-page",
            "2",
        ]);
        let list_args = match args.command {
            Command::Pipeline(PipelineCommand {
                subcommand: PipelineSubcommand::List(options),
            }) => {
                assert_eq!(options.from_page, Some(1));
                assert_eq!(options.to_page, Some(2));
                options
            }
            _ => panic!("Expected PipelineCommand"),
        };
        let options: PipelineOptions = list_args.into();
        match options {
            PipelineOptions::List(args) => {
                assert_eq!(args.from_page, Some(1));
                assert_eq!(args.to_page, Some(2));
            }
            _ => panic!("Expected PipelineOptions::List"),
        }
    }

    #[test]
    fn test_pipeline_cli_runners_list() {
        let args = Args::parse_from(vec![
            "gr",
            "pp",
            "rn",
            "list",
            "online",
            "--tags",
            "tag1,tag2",
            "--all",
            "--from-page",
            "1",
            "--to-page",
            "2",
        ]);
        let list_args = match args.command {
            Command::Pipeline(PipelineCommand {
                subcommand: PipelineSubcommand::Runners(RunnerSubCommand::List(options)),
            }) => {
                assert_eq!(options.status, RunnerStatusCli::Online);
                assert_eq!(
                    options.tags,
                    Some(vec!["tag1".to_string(), "tag2".to_string()])
                );
                assert!(options.all);
                assert_eq!(options.list_args.from_page, Some(1));
                assert_eq!(options.list_args.to_page, Some(2));
                options
            }
            _ => panic!("Expected PipelineCommand"),
        };
        let options: RunnerOptions = list_args.into();
        match options {
            RunnerOptions::List(args) => {
                assert_eq!(args.status, RunnerStatus::Online);
                assert_eq!(args.tags, Some("tag1,tag2".to_string()));
                assert!(args.all);
                assert_eq!(args.list_args.from_page, Some(1));
                assert_eq!(args.list_args.to_page, Some(2));
            }
            _ => panic!("Expected RunnerOptions::List"),
        }
    }

    #[test]
    fn test_get_gitlab_runner_metadata() {
        let args = Args::parse_from(vec!["gr", "pp", "rn", "get", "123"]);
        let list_args = match args.command {
            Command::Pipeline(PipelineCommand {
                subcommand: PipelineSubcommand::Runners(RunnerSubCommand::Get(options)),
            }) => {
                assert_eq!(options.id, 123);
                options
            }
            _ => panic!("Expected PipelineCommand"),
        };
        let options: RunnerOptions = list_args.into();
        match options {
            RunnerOptions::Get(args) => {
                assert_eq!(args.id, 123);
            }
            _ => panic!("Expected RunnerOptions::Get"),
        }
    }

    #[test]
    fn test_pipeline_create_runner() {
        let args = Args::parse_from(vec![
            "gr",
            "pp",
            "rn",
            "create",
            "--description",
            "test-runner",
            "--tags",
            "tag1,tag2",
            "--kind",
            "instance",
        ]);
        let args = match args.command {
            Command::Pipeline(PipelineCommand {
                subcommand: PipelineSubcommand::Runners(RunnerSubCommand::Create(options)),
            }) => {
                assert_eq!(options.description, Some("test-runner".to_string()));
                assert_eq!(
                    options.tags,
                    Some(vec!["tag1".to_string(), "tag2".to_string()])
                );
                assert_eq!(options.kind, RunnerTypeCli::Instance);
                options
            }
            _ => panic!("Expected PipelineCommand"),
        };
        let options: RunnerOptions = args.into();
        match options {
            RunnerOptions::Create(args) => {
                assert_eq!(args.description, Some("test-runner".to_string()));
                assert_eq!(args.tags, Some("tag1,tag2".to_string()));
                assert_eq!(args.kind, RunnerType::Instance);
            }
            _ => panic!("Expected RunnerOptions::Create"),
        }
    }

    #[test]
    fn test_lint_ci_file_args() {
        let args = Args::parse_from(vec!["gr", "pp", "lint"]);
        let options = match args.command {
            Command::Pipeline(PipelineCommand {
                subcommand: PipelineSubcommand::Lint(options),
            }) => {
                assert_eq!(options.path, ".gitlab-ci.yml");
                options
            }
            _ => panic!("Expected PipelineCommand"),
        };
        let options: PipelineOptions = options.into();
        match options {
            PipelineOptions::Lint(args) => {
                assert_eq!(args.path, ".gitlab-ci.yml");
            }
            _ => panic!("Expected PipelineOptions::Lint"),
        }
    }

    #[test]
    fn test_lint_ci_file_args_with_path() {
        let args = Args::parse_from(vec!["gr", "pp", "lint", "path/to/ci.yml"]);
        let options = match args.command {
            Command::Pipeline(PipelineCommand {
                subcommand: PipelineSubcommand::Lint(options),
            }) => {
                assert_eq!(options.path, "path/to/ci.yml");
                options
            }
            _ => panic!("Expected PipelineCommand"),
        };
        let options: PipelineOptions = options.into();
        match options {
            PipelineOptions::Lint(args) => {
                assert_eq!(args.path, "path/to/ci.yml");
            }
            _ => panic!("Expected PipelineOptions::Lint"),
        }
    }

    #[test]
    fn test_merged_ci_file_args() {
        let args = Args::parse_from(vec!["gr", "pp", "merged-ci"]);
        let options = match args.command {
            Command::Pipeline(PipelineCommand {
                subcommand: PipelineSubcommand::MergedCi,
            }) => PipelineOptions::MergedCi,
            _ => panic!("Expected PipelineCommand"),
        };
        match options {
            PipelineOptions::MergedCi => {}
            _ => panic!("Expected PipelineOptions::MergedCi"),
        }
    }

    #[test]
    fn test_chart_cli_args() {
        let args = Args::parse_from(vec!["gr", "pp", "chart"]);
        let options = match args.command {
            Command::Pipeline(PipelineCommand {
                subcommand: PipelineSubcommand::Chart(options),
            }) => {
                assert_eq!(options.chart_type, ChartTypeCli::StagesWithJobs);
                options
            }
            _ => panic!("Expected PipelineCommand"),
        };
        let options: PipelineOptions = options.into();
        match options {
            PipelineOptions::Chart(args) => {
                assert_eq!(args, ChartType::StagesWithJobs);
            }
            _ => panic!("Expected PipelineOptions::Chart"),
        }
    }

    #[test]
    fn test_pipeline_cli_jobs_list() {
        let args = Args::parse_from(vec![
            "gr",
            "pp",
            "jb",
            "list",
            "--from-page",
            "1",
            "--to-page",
            "2",
        ]);

        let list_args = match args.command {
            Command::Pipeline(PipelineCommand {
                subcommand: PipelineSubcommand::Jobs(JobsSubCommand::List(options)),
            }) => {
                assert_eq!(options.list_args.from_page, Some(1));
                assert_eq!(options.list_args.to_page, Some(2));
                options
            }
            _ => panic!("Expected PipelineCommand"),
        };
        let options: JobOptions = list_args.into();
        match options {
            JobOptions::List(args) => {
                assert_eq!(args.list_args.from_page, Some(1));
                assert_eq!(args.list_args.to_page, Some(2));
            }
        }
    }

    #[test]
    fn test_project_runner_with_project_id() {
        let data = RunnerPostData {
            kind: RunnerTypeCli::Project,
            project_id: Some(123),
            group_id: None,
            ..Default::default()
        };
        assert!(data.validate_runner_type_id().is_ok());
    }

    #[test]
    fn test_project_runner_without_project_id() {
        let data = RunnerPostData {
            kind: RunnerTypeCli::Project,
            project_id: None,
            group_id: None,
            ..Default::default()
        };
        assert_eq!(
            data.validate_runner_type_id(),
            Err("error: project id is required for project runner".to_string())
        );
    }

    #[test]
    fn test_group_runner_with_group_id() {
        let data = RunnerPostData {
            kind: RunnerTypeCli::Group,
            project_id: None,
            group_id: Some(456),
            ..Default::default()
        };
        assert!(data.validate_runner_type_id().is_ok());
    }

    #[test]
    fn test_group_runner_without_group_id() {
        let data = RunnerPostData {
            kind: RunnerTypeCli::Group,
            project_id: None,
            group_id: None,
            ..Default::default()
        };
        assert_eq!(
            data.validate_runner_type_id(),
            Err("error: group id is required for group runner".to_string())
        );
    }

    #[test]
    fn test_instance_runner_without_ids() {
        let data = RunnerPostData {
            kind: RunnerTypeCli::Instance,
            project_id: None,
            group_id: None,
            ..Default::default()
        };
        assert!(data.validate_runner_type_id().is_ok());
    }

    #[test]
    fn test_instance_runner_with_project_id() {
        let data = RunnerPostData {
            kind: RunnerTypeCli::Instance,
            project_id: Some(123),
            group_id: None,
            ..Default::default()
        };
        assert_eq!(
            data.validate_runner_type_id(),
            Err("error: project id and group id are not required for instance runner".to_string())
        );
    }

    #[test]
    fn test_instance_runner_with_group_id() {
        let data = RunnerPostData {
            kind: RunnerTypeCli::Instance,
            project_id: None,
            group_id: Some(456),
            ..Default::default()
        };
        assert_eq!(
            data.validate_runner_type_id(),
            Err("error: project id and group id are not required for instance runner".to_string())
        );
    }

    #[test]
    fn test_instance_runner_with_both_ids() {
        let data = RunnerPostData {
            kind: RunnerTypeCli::Instance,
            project_id: Some(123),
            group_id: Some(456),
            ..Default::default()
        };
        assert_eq!(
            data.validate_runner_type_id(),
            Err("error: project id and group id are not required for instance runner".to_string())
        );
    }
}
