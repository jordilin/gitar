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
    #[clap(name = "pp", about = "CI/CD Pipeline operations")]
    Pipeline(PipelineCommand),
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
struct PipelineCommand {
    #[clap(subcommand)]
    pub subcommand: Option<PipelineSubcommand>,
    /// Refresh the cache
    #[clap(long, short)]
    pub refresh: bool,
}

#[derive(Parser)]
enum PipelineSubcommand {
    #[clap(about = "List pipelines")]
    List,
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
        Command::MergeRequest(sub_matches) => Some(CliOptions::MergeRequest(sub_matches.into())),
        Command::Browse(sub_matches) => Some(CliOptions::Browse(sub_matches.into())),
        Command::Pipeline(sub_matches) => Some(CliOptions::Pipeline(sub_matches.into())),
    }
}

pub enum CliOptions {
    MergeRequest(MergeRequestOptions),
    Browse(BrowseOptions),
    Pipeline(PipelineOptions),
}

pub enum BrowseOptions {
    // defaults to open repo in browser
    Repo,
    MergeRequests,
    MergeRequestId(i64),
    Pipelines,
}

pub enum PipelineOptions {
    List { refresh_cache: bool },
}

// From impls - private clap structs to public domain structs
// Mainly to avoid propagating clap further down the stack as changes in the
// clap API could break other parts of the code.

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

impl From<MergeRequestCommand> for MergeRequestOptions {
    fn from(options: MergeRequestCommand) -> Self {
        match options.subcommand {
            MergeRequestSubcommand::Create(options) => options.into(),
            MergeRequestSubcommand::List(options) => options.into(),
            MergeRequestSubcommand::Merge(options) => options.into(),
            MergeRequestSubcommand::Checkout(options) => options.into(),
            MergeRequestSubcommand::Close(options) => options.into(),
        }
    }
}

impl From<MergeRequestBrowse> for BrowseOptions {
    fn from(options: MergeRequestBrowse) -> Self {
        match options.id {
            Some(id) => BrowseOptions::MergeRequestId(id),
            None => BrowseOptions::MergeRequests,
        }
    }
}

impl From<BrowseCommand> for BrowseOptions {
    fn from(options: BrowseCommand) -> Self {
        match options.subcommand {
            Some(BrowseSubcommand::Repo) => BrowseOptions::Repo,
            Some(BrowseSubcommand::MergeRequest(options)) => options.into(),
            Some(BrowseSubcommand::Pipelines) => BrowseOptions::Pipelines,
            // defaults to open repo in browser
            None => BrowseOptions::Repo,
        }
    }
}

impl From<PipelineCommand> for PipelineOptions {
    fn from(options: PipelineCommand) -> Self {
        match options.subcommand {
            Some(PipelineSubcommand::List) => PipelineOptions::List {
                refresh_cache: options.refresh,
            },
            // defaults to list all pipelines
            None => PipelineOptions::List {
                refresh_cache: options.refresh,
            },
        }
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
