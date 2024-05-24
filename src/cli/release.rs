use clap::Parser;

use crate::{cmds::release::ReleaseAssetListCliArgs, remote::ListRemoteCliArgs};

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
    #[clap(subcommand, about = "Release assets operations")]
    Assets(ReleaseAssetSubcommand),
}

#[derive(Parser)]
pub enum ReleaseAssetSubcommand {
    #[clap(about = "List release assets")]
    List(ListAssets),
}

#[derive(Parser)]
pub struct ListAssets {
    /// Release ID
    #[clap()]
    release_id: i64,
    #[command(flatten)]
    list_args: ListArgs,
}

impl From<ReleaseCommand> for ReleaseOptions {
    fn from(options: ReleaseCommand) -> Self {
        match options.subcommand {
            ReleaseSubcommand::List(options) => options.into(),
            ReleaseSubcommand::Assets(subcommand) => match subcommand {
                ReleaseAssetSubcommand::List(options) => ReleaseOptions::Assets(options.into()),
            },
        }
    }
}

impl From<ListArgs> for ReleaseOptions {
    fn from(args: ListArgs) -> Self {
        ReleaseOptions::List(args.into())
    }
}

impl From<ReleaseAssetSubcommand> for ReleaseAssetOptions {
    fn from(subcommand: ReleaseAssetSubcommand) -> Self {
        match subcommand {
            ReleaseAssetSubcommand::List(options) => ReleaseAssetOptions::List(options.into()),
        }
    }
}

impl From<ListAssets> for ReleaseAssetOptions {
    fn from(args: ListAssets) -> Self {
        ReleaseAssetOptions::List(args.into())
    }
}

impl From<ListAssets> for ReleaseAssetListCliArgs {
    fn from(args: ListAssets) -> Self {
        ReleaseAssetListCliArgs::builder()
            .id(args.release_id)
            .list_args(args.list_args.into())
            .build()
            .unwrap()
    }
}

pub enum ReleaseOptions {
    List(ListRemoteCliArgs),
    Assets(ReleaseAssetOptions),
}

pub enum ReleaseAssetOptions {
    List(ReleaseAssetListCliArgs),
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
        let list_args = match args.command {
            Command::Release(ReleaseCommand {
                subcommand: ReleaseSubcommand::List(options),
            }) => {
                assert_eq!(options.from_page, Some(1));
                assert_eq!(options.to_page, Some(2));
                options
            }
            _ => panic!("Expected ReleaseCommand"),
        };
        let options: ReleaseOptions = list_args.into();
        match options {
            ReleaseOptions::List(args) => {
                assert_eq!(args.from_page, Some(1));
                assert_eq!(args.to_page, Some(2));
            }
            _ => panic!("Expected ReleaseOptions::List"),
        }
    }

    #[test]
    fn test_release_asset_cli_list() {
        let args = Args::parse_from(vec![
            "gr",
            "rl",
            "assets",
            "list",
            "1",
            "--from-page",
            "1",
            "--to-page",
            "2",
        ]);
        let list_args = match args.command {
            Command::Release(ReleaseCommand {
                subcommand: ReleaseSubcommand::Assets(ReleaseAssetSubcommand::List(options)),
            }) => {
                assert_eq!(options.release_id, 1);
                assert_eq!(options.list_args.from_page, Some(1));
                assert_eq!(options.list_args.to_page, Some(2));
                options
            }
            _ => panic!("Expected ReleaseAssetSubcommand::List"),
        };
        let options: ReleaseAssetOptions = list_args.into();
        match options {
            ReleaseAssetOptions::List(args) => {
                assert_eq!(args.id, 1);
                assert_eq!(args.list_args.from_page, Some(1));
                assert_eq!(args.list_args.to_page, Some(2));
            }
        }
    }
}
