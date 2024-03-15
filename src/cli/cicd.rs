use clap::Parser;

use crate::remote::ListRemoteCliArgs;

use super::common::{gen_list_args, ListArgs};

#[derive(Parser)]
pub struct PipelineCommand {
    #[clap(subcommand)]
    subcommand: PipelineSubcommand,
}

#[derive(Parser)]
enum PipelineSubcommand {
    #[clap(about = "List pipelines")]
    List(ListArgs),
}

impl From<PipelineCommand> for PipelineOptions {
    fn from(options: PipelineCommand) -> Self {
        match options.subcommand {
            PipelineSubcommand::List(options) => options.into(),
        }
    }
}

impl From<ListArgs> for PipelineOptions {
    fn from(options: ListArgs) -> Self {
        PipelineOptions::List(gen_list_args(options))
    }
}

pub enum PipelineOptions {
    List(ListRemoteCliArgs),
}
