use clap::Parser;

use crate::cmds::merge_request::MergeRequestListCliArgs;

use super::common::gen_list_args;
use super::merge_request::ListMergeRequest;

#[derive(Parser)]
pub struct MyCommand {
    #[clap(subcommand)]
    subcommand: MySubcommand,
}

#[derive(Parser)]
enum MySubcommand {
    #[clap(about = "Lists your assigned merge requests", name = "mr")]
    MergeRequest(ListMergeRequest),
}

pub enum MyOptions {
    MergeRequest(MergeRequestListCliArgs),
}

impl From<MyCommand> for MyOptions {
    fn from(options: MyCommand) -> Self {
        match options.subcommand {
            MySubcommand::MergeRequest(options) => options.into(),
        }
    }
}

impl From<ListMergeRequest> for MyOptions {
    fn from(options: ListMergeRequest) -> Self {
        let list_args = gen_list_args(options.list_args);
        MyOptions::MergeRequest(MergeRequestListCliArgs::new(
            options.state.into(),
            list_args,
            true,
        ))
    }
}
