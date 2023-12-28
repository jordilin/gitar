use crate::remote::MergeRequestState;
use clap::{Parser, ValueEnum};

#[derive(Parser)]
#[command(about = "A Github/Gitlab CLI tool")]
struct Args {
    #[clap(subcommand)]
    pub command: Command,
}

#[derive(Parser)]
enum Command {
    #[clap(name = "mr", about = "Merge request operations")]
    MergeRequest(MergeRequestCommand),
    #[clap(name = "br", about = "Open the remote using your browser")]
    Browse(BrowseCommand),
}

#[derive(Parser)]
struct MergeRequestCommand {
    #[clap(subcommand)]
    pub subcommand: MergeRequestSubcommand,
}

#[derive(Parser)]
enum MergeRequestSubcommand {
    #[clap(about = "Creates a merge request")]
    Create(CreateMergeRequest),
    #[clap(about = "List merge requests")]
    List(ListMergeRequest),
    #[clap(about = "Merge a merge request")]
    Merge(MergeMergeRequest),
    #[clap(about = "Git checkout a merge request branch for review")]
    Checkout(CheckoutMergeRequest),
    #[clap(about = "Close a merge request")]
    Close(CloseMergeRequest),
}

#[derive(Parser)]
struct CreateMergeRequest {
    /// Title of the merge request
    #[clap(long)]
    pub title: Option<String>,
    /// Description of the merge request
    #[clap(long)]
    pub description: Option<String>,
    /// Do not prompt for confirmation
    #[clap(long)]
    pub auto: bool,
    /// Target branch of the merge request instead of default project's upstream branch
    #[clap(long)]
    pub target_branch: Option<String>,
    /// Refresh the cache
    #[clap(long, short)]
    pub refresh: bool,
}

#[derive(ValueEnum, Clone)]
pub enum MergeRequestStateStateCli {
    Opened,
    Closed,
    Merged,
}

impl From<MergeRequestStateStateCli> for MergeRequestState {
    fn from(state: MergeRequestStateStateCli) -> Self {
        match state {
            MergeRequestStateStateCli::Opened => MergeRequestState::Opened,
            MergeRequestStateStateCli::Closed => MergeRequestState::Closed,
            MergeRequestStateStateCli::Merged => MergeRequestState::Merged,
        }
    }
}

#[derive(Parser)]
struct ListMergeRequest {
    #[clap()]
    pub state: MergeRequestStateStateCli,
    /// Refresh the cache
    #[clap(long, short)]
    pub refresh: bool,
}

#[derive(Parser)]
struct MergeMergeRequest {
    /// Id of the merge request
    #[clap()]
    pub id: i64,
}

#[derive(Parser)]
struct CheckoutMergeRequest {
    /// Id of the merge request
    #[clap()]
    pub id: i64,
}

#[derive(Parser)]
struct CloseMergeRequest {
    /// Id of the merge request
    #[clap()]
    pub id: i64,
}

#[derive(Parser)]
struct BrowseCommand {
    #[clap(subcommand)]
    pub subcommand: Option<BrowseSubcommand>,
}

#[derive(Parser)]
enum BrowseSubcommand {
    #[clap(about = "Open the repo using your browser")]
    Repo,
    #[clap(name = "mr", about = "Open the merge requests using your browser")]
    MergeRequest(MergeRequestBrowse),
    #[clap(name = "pp", about = "Open the ci/cd pipelines using your browser")]
    Pipelines,
}

#[derive(Parser)]
struct MergeRequestBrowse {
    /// Open merge/pull request id in the browser
    #[clap()]
    pub id: Option<i64>,
}

// Parse cli and return CliOptions
pub fn parse_cli() -> Option<CliOptions> {
    let args = Args::parse();
    match args.command {
        Command::MergeRequest(sub_matches) => match sub_matches.subcommand {
            MergeRequestSubcommand::Create(sub_matches) => {
                return Some(CliOptions::MergeRequest(sub_matches.into()));
            }
            MergeRequestSubcommand::List(sub_matches) => {
                return Some(CliOptions::MergeRequest(sub_matches.into()));
            }
            MergeRequestSubcommand::Merge(sub_matches) => {
                return Some(CliOptions::MergeRequest(sub_matches.into()));
            }

            MergeRequestSubcommand::Checkout(sub_matches) => {
                return Some(CliOptions::MergeRequest(sub_matches.into()));
            }
            MergeRequestSubcommand::Close(sub_matches) => {
                return Some(CliOptions::MergeRequest(sub_matches.into()));
            }
        },
        Command::Browse(sub_matches) => {
            let br_cmd = sub_matches.subcommand.unwrap_or(BrowseSubcommand::Repo);
            match br_cmd {
                BrowseSubcommand::Repo => {
                    return Some(CliOptions::Browse(BrowseOptions::Repo));
                }
                BrowseSubcommand::MergeRequest(sub_matches) => {
                    if let Some(id) = sub_matches.id {
                        return Some(CliOptions::Browse(BrowseOptions::MergeRequestId(id)));
                    }
                    return Some(CliOptions::Browse(BrowseOptions::MergeRequests));
                }
                BrowseSubcommand::Pipelines => {
                    return Some(CliOptions::Browse(BrowseOptions::Pipelines));
                }
            }
        }
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
    Pipelines,
}

impl From<CreateMergeRequest> for MergeRequestOptions {
    fn from(options: CreateMergeRequest) -> Self {
        MergeRequestOptions::Create {
            title: options.title,
            description: options.description,
            target_branch: options.target_branch,
            noprompt: options.auto,
            refresh_cache: options.refresh,
        }
    }
}

impl From<ListMergeRequest> for MergeRequestOptions {
    fn from(options: ListMergeRequest) -> Self {
        MergeRequestOptions::List {
            state: options.state.into(),
            refresh_cache: options.refresh,
        }
    }
}

impl From<MergeMergeRequest> for MergeRequestOptions {
    fn from(options: MergeMergeRequest) -> Self {
        MergeRequestOptions::Merge { id: options.id }
    }
}

impl From<CheckoutMergeRequest> for MergeRequestOptions {
    fn from(options: CheckoutMergeRequest) -> Self {
        MergeRequestOptions::Checkout { id: options.id }
    }
}

impl From<CloseMergeRequest> for MergeRequestOptions {
    fn from(options: CloseMergeRequest) -> Self {
        MergeRequestOptions::Close { id: options.id }
    }
}

pub enum MergeRequestOptions {
    Create {
        title: Option<String>,
        description: Option<String>,
        target_branch: Option<String>,
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
