use clap::{Parser, ValueEnum};

use crate::{
    cmds::cicd::{LintFilePathArgs, RunnerListCliArgs, RunnerMetadataGetCliArgs, RunnerStatus},
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
    Chart,
    #[clap(about = "List pipelines")]
    List(ListArgs),
    #[clap(subcommand, name = "rn", about = "Runner operations")]
    Runners(RunnerSubCommand),
}

#[derive(Parser)]
struct FilePathArgs {
    /// Path to the ci yml file.
    #[clap(default_value = ".gitlab-ci.yml")]
    path: String,
}

#[derive(Parser)]
enum RunnerSubCommand {
    #[clap(about = "List runners")]
    List(ListRunner),
    #[clap(about = "Get runner metadata")]
    Get(RunnerMetadata),
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

impl From<PipelineCommand> for PipelineOptions {
    fn from(options: PipelineCommand) -> Self {
        match options.subcommand {
            PipelineSubcommand::Lint(options) => options.into(),
            PipelineSubcommand::MergedCi => PipelineOptions::MergedCi,
            PipelineSubcommand::Chart => PipelineOptions::Chart,
            PipelineSubcommand::List(options) => options.into(),
            PipelineSubcommand::Runners(options) => options.into(),
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

pub enum PipelineOptions {
    Lint(LintFilePathArgs),
    List(ListRemoteCliArgs),
    Runners(RunnerOptions),
    MergedCi,
    Chart,
}

pub enum RunnerOptions {
    List(RunnerListCliArgs),
    Get(RunnerMetadataGetCliArgs),
}

#[cfg(test)]
mod test {
    use crate::cli::{Args, Command};

    use super::*;

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
                assert_eq!(options.all, true);
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
                assert_eq!(args.all, true);
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
                subcommand: PipelineSubcommand::Chart,
            }) => PipelineOptions::MergedCi,
            _ => panic!("Expected PipelineCommand"),
        };
        match options {
            PipelineOptions::MergedCi => {}
            _ => panic!("Expected PipelineOptions::Chart"),
        }
    }
}
