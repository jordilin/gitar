use clap::Parser;

use crate::remote::ListRemoteCliArgs;

use super::common::ListArgs;

#[derive(Parser)]
pub struct ReleaseCommand {
    #[clap(subcommand)]
    pub subcommand: ReleaseSubcommand,
}

#[derive(Parser)]
pub enum ReleaseSubcommand {
    #[clap(about = "List releases")]
    List(ListArgs),
}

impl From<ReleaseCommand> for ReleaseOptions {
    fn from(options: ReleaseCommand) -> Self {
        match options.subcommand {
            ReleaseSubcommand::List(options) => options.into(),
        }
    }
}

impl From<ListArgs> for ReleaseOptions {
    fn from(args: ListArgs) -> Self {
        ReleaseOptions::List(args.into())
    }
}

pub enum ReleaseOptions {
    List(ListRemoteCliArgs),
}

#[cfg(test)]
mod test {
    use crate::cli::{Args, Command};

    use super::*;

    #[test]
    fn test_release_cli_list() {
        let args = Args::parse_from(vec![
            "gr",
            "rl",
            "list",
            "--from-page",
            "1",
            "--to-page",
            "2",
        ]);
        match args.command {
            Command::Release(ReleaseCommand {
                subcommand: ReleaseSubcommand::List(options),
            }) => {
                assert_eq!(options.from_page, Some(1));
                assert_eq!(options.to_page, Some(2));
            }
            _ => panic!("Expected ReleaseCommand"),
        }
    }
}
