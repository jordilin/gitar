use clap::{arg, builder::PossibleValue, value_parser, ArgAction, Command};

use crate::remote::MergeRequestState;

fn cli() -> Command {
    Command::new("gr")
        .about("A Github/Gitlab CLI tool")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .arg(
            arg!(-r --refresh "Refresh the cache")
                .action(ArgAction::SetTrue)
                .required(false),
        )
        .subcommand(merge_request_command())
        .subcommand(browse_command())
}

fn merge_request_command() -> Command {
    Command::new("mr")
        .about("Merge request operations")
        .subcommand(
            Command::new("create")
                .about("Creates a merge request")
                .arg(arg!(--title <TITLE>  "Title of the merge request").required(false))
                .arg(
                    arg!(--description <DESCRIPTION> "Description of the merge request")
                        .required(false),
                )
                .arg(
                    arg!(--auto "Do not prompt for confirmation")
                        .action(ArgAction::SetTrue)
                        .required(false),
                ),
        )
        .subcommand(
            Command::new("list").about("List merge requests").arg(
                arg!(--state <STATE> "State of the merge request")
                    .value_parser([
                        PossibleValue::new("opened"),
                        PossibleValue::new("closed"),
                        PossibleValue::new("merged"),
                    ])
                    .required(true),
            ),
        )
        .subcommand(
            Command::new("merge").about("Merge a merge request").arg(
                arg!(<ID> "Id of the merge request")
                    .value_parser(value_parser!(i64))
                    .required(true),
            ),
        )
        .subcommand(merge_request_checkout_command())
        .subcommand(merge_request_close_command())
}

fn merge_request_checkout_command() -> Command {
    Command::new("checkout")
        .about("Git checkout a merge request branch for review")
        .arg(
            arg!(<ID> "Id of the merge request")
                .value_parser(value_parser!(i64))
                .required(true),
        )
}

fn merge_request_close_command() -> Command {
    Command::new("close").about("Close a merge request").arg(
        arg!(<ID> "Id of the merge request")
            .value_parser(value_parser!(i64))
            .required(true),
    )
}

fn browse_command() -> Command {
    Command::new("br")
        .about(
            "Open the remote using your browser. If no command is specified, it will open the repo",
        )
        .subcommand(browse_repo_subcommand())
        .subcommand(browse_mr_subcommand())
}

fn browse_repo_subcommand() -> Command {
    Command::new("repo").about("Open the repo using your browser")
}

fn browse_mr_subcommand() -> Command {
    Command::new("mr")
        .about("Open the merge requests using your browser")
        .arg(
            arg!(<ID> "Open merge/pull request id in the browser")
                .value_parser(value_parser!(i64))
                .required(false),
        )
}

// Parse cli and return CliOptions
pub fn parse_cli() -> Option<CliOptions> {
    let matches = cli().get_matches();
    let refresh_cache = matches.get_flag("refresh");
    match matches.subcommand() {
        Some(("mr", sub_matches)) => match sub_matches.subcommand() {
            Some(("create", sub_matches)) => {
                let title = sub_matches.get_one::<String>("title");
                let description = sub_matches.get_one::<String>("description");
                let noprompt = sub_matches.get_flag("auto");
                return Some(CliOptions::MergeRequest(MergeRequestOptions::Create {
                    title: title.as_ref().map(|s| s.to_string()),
                    description: description.as_ref().map(|s| s.to_string()),
                    noprompt,
                    refresh_cache,
                }));
            }
            Some(("list", sub_matches)) => {
                return Some(CliOptions::MergeRequest(MergeRequestOptions::List {
                    state: match sub_matches.get_one::<String>("state") {
                        Some(s) => s.as_str().try_into().unwrap(),
                        None => {
                            eprintln!("Please specify a state");
                            std::process::exit(1);
                        }
                    },
                    refresh_cache,
                }));
            }
            Some(("merge", sub_matches)) => {
                return Some(CliOptions::MergeRequest(MergeRequestOptions::Merge {
                    id: *sub_matches
                        .get_one::<i64>("ID")
                        .expect("Please specify an id"),
                }));
            }
            Some(("checkout", sub_matches)) => {
                return Some(CliOptions::MergeRequest(MergeRequestOptions::Checkout {
                    id: *sub_matches
                        .get_one::<i64>("ID")
                        .expect("Please specify an id"),
                }));
            }
            Some(("close", sub_matches)) => {
                return Some(CliOptions::MergeRequest(MergeRequestOptions::Close {
                    id: *sub_matches
                        .get_one::<i64>("ID")
                        .expect("Please specify an id"),
                }));
            }
            _ => None,
        },
        Some(("br", sub_matches)) => match sub_matches.subcommand() {
            Some(("repo", _)) => Some(CliOptions::Browse(BrowseOptions::Repo)),
            Some(("mr", sub_matches)) => {
                if let Some(id) = sub_matches.get_one::<i64>("ID") {
                    return Some(CliOptions::Browse(BrowseOptions::MergeRequestId(*id)));
                }
                Some(CliOptions::Browse(BrowseOptions::MergeRequests))
            }
            _ => {
                // default open remote repo in browser
                Some(CliOptions::Browse(BrowseOptions::Repo))
            }
        },
        _ => None,
    }
}

pub enum CliOptions {
    MergeRequest(MergeRequestOptions),
    Browse(BrowseOptions),
}

pub enum BrowseOptions {
    // defaults to open repo in browser
    Repo,
    MergeRequests,
    MergeRequestId(i64),
    // TODO Pipelines/Actions, close MRs
}

pub enum MergeRequestOptions {
    Create {
        title: Option<String>,
        description: Option<String>,
        noprompt: bool,
        refresh_cache: bool,
    },
    List {
        state: MergeRequestState,
        refresh_cache: bool,
    },
    Merge {
        id: i64,
    },
    Checkout {
        id: i64,
    },
    Close {
        id: i64,
    },
}
