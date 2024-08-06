use clap::Parser;

use crate::cmds::{
    gist::GistListCliArgs,
    merge_request::{MergeRequestListCliArgs, MergeRequestUser},
    project::ProjectListCliArgs,
};

use super::{common::ListArgs, merge_request::ListMergeRequest, project::ListProject};

#[derive(Parser)]
pub struct MyCommand {
    #[clap(subcommand)]
    subcommand: MySubcommand,
}

#[derive(Parser)]
enum MySubcommand {
    #[clap(
        about = "Lists the merge requests where you are the author, assignee or the reviewer",
        name = "mr"
    )]
    MergeRequest(ListMyMergeRequest),
    #[clap(about = "Lists your projects", name = "pj")]
    Project(ListProject),
    #[clap(about = "Lists your starred projects", name = "st")]
    Star(ListStar),
    #[clap(about = "Lists your gists", name = "gs")]
    Gist(ListGist),
}

#[derive(Parser)]
struct ListMyMergeRequest {
    /// Filter merge requests where you are the assignee. Gitlab and Github.
    #[clap(long, group = "merge_request")]
    assignee: bool,
    /// Filter merge requests where you are the author. Default if none
    /// provided. Gitlab and Github.
    #[clap(long, group = "merge_request")]
    author: bool,
    /// Filter merge requests where you are the reviewer. Gitlab only.
    #[clap(long, group = "merge_request")]
    reviewer: bool,
    #[clap(flatten)]
    list_merge_request: ListMergeRequest,
}

pub enum MyOptions {
    MergeRequest(MergeRequestListCliArgs),
    Project(ProjectListCliArgs),
    Gist(GistListCliArgs),
}

impl From<MyCommand> for MyOptions {
    fn from(options: MyCommand) -> Self {
        match options.subcommand {
            MySubcommand::MergeRequest(options) => options.into(),
            MySubcommand::Project(options) => options.into(),
            MySubcommand::Star(options) => options.into(),
            MySubcommand::Gist(options) => options.into(),
        }
    }
}

impl From<ListMyMergeRequest> for MyOptions {
    fn from(options: ListMyMergeRequest) -> Self {
        MyOptions::MergeRequest(
            MergeRequestListCliArgs::builder()
                .state(options.list_merge_request.state.into())
                .list_args(options.list_merge_request.list_args.into())
                .assignee(if options.assignee {
                    Some(MergeRequestUser::Me)
                } else {
                    None
                })
                // Author is the default if none is provided.
                .author(
                    if options.author || (!options.assignee && !options.reviewer) {
                        Some(MergeRequestUser::Me)
                    } else {
                        None
                    },
                )
                .reviewer(if options.reviewer {
                    Some(MergeRequestUser::Me)
                } else {
                    None
                })
                .build()
                .unwrap(),
        )
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

#[derive(Parser)]
pub struct ListGist {
    #[clap(flatten)]
    pub list_args: ListArgs,
}

impl From<ListGist> for MyOptions {
    fn from(options: ListGist) -> Self {
        MyOptions::Gist(
            GistListCliArgs::builder()
                .list_args(options.list_args.into())
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
        cmds::merge_request::MergeRequestState,
    };

    #[test]
    fn test_my_merge_request_cli_args() {
        let args = Args::parse_from(vec!["gr", "my", "mr", "opened"]);
        let my_command = match args.command {
            Command::My(MyCommand {
                subcommand: MySubcommand::MergeRequest(options),
            }) => {
                assert_eq!(
                    options.list_merge_request.state,
                    MergeRequestStateStateCli::Opened
                );
                options
            }
            _ => panic!("Expected MyCommand"),
        };
        let options: MyOptions = my_command.into();
        match options {
            MyOptions::MergeRequest(options) => {
                assert_eq!(options.state, MergeRequestState::Opened);
                assert_eq!(options.author, Some(MergeRequestUser::Me));
            }
            _ => panic!("Expected MyOptions::MergeRequest"),
        }
    }

    #[test]
    fn test_my_merge_request_cli_args_reviewer() {
        let args = Args::parse_from(vec!["gr", "my", "mr", "opened", "--reviewer"]);
        let my_command = match args.command {
            Command::My(MyCommand {
                subcommand: MySubcommand::MergeRequest(options),
            }) => {
                assert_eq!(
                    options.list_merge_request.state,
                    MergeRequestStateStateCli::Opened
                );
                assert!(options.reviewer);
                options
            }
            _ => panic!("Expected MyCommand"),
        };
        let options: MyOptions = my_command.into();
        match options {
            MyOptions::MergeRequest(options) => {
                assert_eq!(options.state, MergeRequestState::Opened);
                assert_eq!(options.reviewer, Some(MergeRequestUser::Me));
            }
            _ => panic!("Expected MyOptions::MergeRequest"),
        }
    }

    #[test]
    fn test_my_merge_request_cli_args_author() {
        let args = Args::parse_from(vec!["gr", "my", "mr", "opened", "--author"]);
        let my_command = match args.command {
            Command::My(MyCommand {
                subcommand: MySubcommand::MergeRequest(options),
            }) => {
                assert_eq!(
                    options.list_merge_request.state,
                    MergeRequestStateStateCli::Opened
                );
                assert!(options.author);
                options
            }
            _ => panic!("Expected MyCommand"),
        };
        let options: MyOptions = my_command.into();
        match options {
            MyOptions::MergeRequest(options) => {
                assert_eq!(options.state, MergeRequestState::Opened);
                assert_eq!(options.author, Some(MergeRequestUser::Me));
            }
            _ => panic!("Expected MyOptions::MergeRequest"),
        }
    }

    #[test]
    fn test_my_merge_request_cli_args_assignee() {
        let args = Args::parse_from(vec!["gr", "my", "mr", "opened", "--assignee"]);
        let my_command = match args.command {
            Command::My(MyCommand {
                subcommand: MySubcommand::MergeRequest(options),
            }) => {
                assert_eq!(
                    options.list_merge_request.state,
                    MergeRequestStateStateCli::Opened
                );
                assert!(options.assignee);
                options
            }
            _ => panic!("Expected MyCommand"),
        };
        let options: MyOptions = my_command.into();
        match options {
            MyOptions::MergeRequest(options) => {
                assert_eq!(options.state, MergeRequestState::Opened);
                assert_eq!(options.assignee, Some(MergeRequestUser::Me));
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

    #[test]
    fn test_my_gists_cli_args() {
        let args = Args::parse_from(vec!["gr", "my", "gs"]);
        let my_command = match args.command {
            Command::My(MyCommand {
                subcommand: MySubcommand::Gist(options),
            }) => options,
            _ => panic!("Expected MyCommand"),
        };
        let options: MyOptions = my_command.into();
        match options {
            MyOptions::Gist(_) => {}
            _ => panic!("Expected MyOptions::Gist"),
        }
    }
}
