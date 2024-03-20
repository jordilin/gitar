use clap::{Parser, ValueEnum};

use crate::{
    cmds::cicd::{RunnerListCliArgs, RunnerMetadataCliArgs, RunnerStatus},
    remote::ListRemoteCliArgs,
};

use super::common::{gen_list_args, FormatCli, ListArgs};

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
}

#[derive(Parser)]
struct ListRunner {
    /// Runner status
    #[clap()]
    status: RunnerStatusCli,
    /// Comma separated list of tags
    #[clap(long, value_delimiter = ',')]
    tags: Option<Vec<String>>,
    #[command(flatten)]
    list_args: ListArgs,
}

#[derive(Parser)]
struct RunnerMetadata {
    /// Runner ID
    #[clap()]
    id: i64,
    /// Refresh the cache
    #[clap(long, short)]
    pub refresh: bool,
    /// Do not print headers
    #[clap(long)]
    pub no_headers: bool,
    /// Output format
    #[clap(long, default_value_t=FormatCli::Pipe)]
    format: FormatCli,
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
        PipelineOptions::List(gen_list_args(options))
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
        }
    }
}

impl From<ListRunner> for RunnerOptions {
    fn from(options: ListRunner) -> Self {
        let tags = if let Some(tags) = options.tags {
            Some(tags.into_iter().map(|tag| tag.trim().to_string()).collect())
        } else {
            None
        };
        RunnerOptions::List(
            RunnerListCliArgs::builder()
                .status(options.status.into())
                .tags(tags)
                .list_args(gen_list_args(options.list_args))
                .build()
                .unwrap(),
        )
    }
}

impl From<RunnerMetadata> for RunnerOptions {
    fn from(options: RunnerMetadata) -> Self {
        RunnerOptions::Get(
            RunnerMetadataCliArgs::builder()
                .id(options.id)
                .refresh_cache(options.refresh)
                .no_headers(options.no_headers)
                .format(options.format.into())
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
    Get(RunnerMetadataCliArgs),
}
