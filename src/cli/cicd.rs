use clap::{Parser, ValueEnum};

use crate::{
    cmds::cicd::{RunnerListCliArgs, RunnerMetadataGetCliArgs, RunnerStatus},
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
    #[clap(about = "List pipelines")]
    List(ListArgs),
    #[clap(subcommand, name = "rn", about = "Runner operations")]
    Runners(RunnerSubCommand),
}

#[derive(Parser)]
enum RunnerSubCommand {
    #[clap(about = "List runners")]
    List(ListRunner),
    #[clap(about = "Get runner metadata")]
    Get(RunnerMetadata),
}

#[derive(ValueEnum, Clone)]
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
    #[clap(long, value_delimiter = ',')]
    tags: Option<Vec<String>>,
    /// List all runners available across all projects. Gitlab admins only.
    #[clap(long)]
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
            PipelineSubcommand::List(options) => options.into(),
            PipelineSubcommand::Runners(options) => options.into(),
        }
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
    List(ListRemoteCliArgs),
    Runners(RunnerOptions),
}

pub enum RunnerOptions {
    List(RunnerListCliArgs),
    Get(RunnerMetadataGetCliArgs),
}
