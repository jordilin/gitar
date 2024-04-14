use clap::Parser;

use crate::cmds::{merge_request::MergeRequestListCliArgs, project::ProjectListCliArgs};

use super::{common::ListArgs, merge_request::ListMergeRequest, project::ListProject};

#[derive(Parser)]
pub struct MyCommand {
    #[clap(subcommand)]
    subcommand: MySubcommand,
}

#[derive(Parser)]
enum MySubcommand {
    #[clap(about = "Lists your assigned merge requests", name = "mr")]
    MergeRequest(ListMergeRequest),
    #[clap(about = "Lists your projects", name = "pj")]
    Project(ListProject),
    #[clap(about = "Lists your starred projects", name = "st")]
    Star(ListStar),
}

pub enum MyOptions {
    MergeRequest(MergeRequestListCliArgs),
    Project(ProjectListCliArgs),
}

impl From<MyCommand> for MyOptions {
    fn from(options: MyCommand) -> Self {
        match options.subcommand {
            MySubcommand::MergeRequest(options) => options.into(),
            MySubcommand::Project(options) => options.into(),
            MySubcommand::Star(options) => options.into(),
        }
    }
}

impl From<ListMergeRequest> for MyOptions {
    fn from(options: ListMergeRequest) -> Self {
        MyOptions::MergeRequest(MergeRequestListCliArgs::new(
            options.state.into(),
            options.list_args.into(),
        ))
    }
}

impl From<ListProject> for MyOptions {
    fn from(options: ListProject) -> Self {
        MyOptions::Project(
            ProjectListCliArgs::builder()
                .list_args(options.list_args.into())
                .build()
                .unwrap(),
        )
    }
}

#[derive(Parser)]
pub struct ListStar {
    #[clap(flatten)]
    pub list_args: ListArgs,
}

impl From<ListStar> for MyOptions {
    fn from(options: ListStar) -> Self {
        MyOptions::Project(
            ProjectListCliArgs::builder()
                .list_args(options.list_args.into())
                .stars(true)
                .build()
                .unwrap(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        cli::{merge_request::MergeRequestStateStateCli, Args, Command},
        remote::MergeRequestState,
    };

    #[test]
    fn test_my_merge_request_cli_args() {
        let args = Args::parse_from(vec!["gr", "my", "mr", "opened"]);
        let my_command = match args.command {
            Command::My(MyCommand {
                subcommand: MySubcommand::MergeRequest(options),
            }) => {
                assert_eq!(options.state, MergeRequestStateStateCli::Opened);
                options
            }
            _ => panic!("Expected MyCommand"),
        };
        let options: MyOptions = my_command.into();
        match options {
            MyOptions::MergeRequest(options) => {
                assert_eq!(options.state, MergeRequestState::Opened);
            }
            _ => panic!("Expected MyOptions::MergeRequest"),
        }
    }

    #[test]
    fn test_my_projects_cli_args() {
        let args = Args::parse_from(vec!["gr", "my", "pj"]);
        let my_command = match args.command {
            Command::My(MyCommand {
                subcommand: MySubcommand::Project(options),
            }) => options,
            _ => panic!("Expected MyCommand"),
        };
        let options: MyOptions = my_command.into();
        match options {
            MyOptions::Project(_) => {}
            _ => panic!("Expected MyOptions::Project"),
        }
    }

    #[test]
    fn test_my_stars_cli_args() {
        let args = Args::parse_from(vec!["gr", "my", "st"]);
        let my_command = match args.command {
            Command::My(MyCommand {
                subcommand: MySubcommand::Star(options),
            }) => options,
            _ => panic!("Expected MyCommand"),
        };
        let options: MyOptions = my_command.into();
        match options {
            MyOptions::Project(_) => {}
            _ => panic!("Expected MyOptions::Star"),
        }
    }
}
