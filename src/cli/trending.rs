use clap::Parser;

use crate::cmds::trending::TrendingCliArgs;

use super::common::GetArgs;

#[derive(Parser)]
pub struct TrendingCommand {
    #[clap()]
    pub language: String,
    #[clap(flatten)]
    get_args: GetArgs,
}

pub enum TrendingOptions {
    Get(TrendingCliArgs),
}

impl From<TrendingCommand> for TrendingOptions {
    fn from(options: TrendingCommand) -> Self {
        TrendingOptions::Get(TrendingCliArgs {
            language: options.language,
            get_args: options.get_args.into(),
            flush: false,
        })
    }
}

impl From<TrendingOptions> for TrendingCliArgs {
    fn from(options: TrendingOptions) -> Self {
        match options {
            TrendingOptions::Get(args) => args,
        }
    }
}
