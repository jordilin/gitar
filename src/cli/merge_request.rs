use std::option::Option;

use clap::{Parser, ValueEnum};

use crate::{
    cmds::merge_request::{MergeRequestCliArgs, MergeRequestListCliArgs},
    remote::MergeRequestState,
};
use common::ListArgs;

use super::common::{self, gen_list_args};

#[derive(Parser)]
pub struct MergeRequestCommand {
    #[clap(subcommand)]
    subcommand: MergeRequestSubcommand,
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
    #[clap(long, group = "title_msg")]
    pub title: Option<String>,
    /// Gather title and description from the specified commit message
    #[clap(long, group = "title_msg", value_name = "SHA")]
    pub title_from_commit: Option<String>,
    /// Description of the merge request
    #[clap(long)]
    pub description: Option<String>,
    /// Gather merge request description from the specified file. If "-" is
    /// provided, read from STDIN
    #[clap(long, value_name = "FILE")]
    pub description_from_file: Option<String>,
    /// Accept the default title, description, and target branch
    #[clap(long)]
    pub auto: bool,
    /// Target branch of the merge request instead of default project's upstream branch
    #[clap(long)]
    pub target_branch: Option<String>,
    /// Refresh the cache
    #[clap(long, short)]
    pub refresh: bool,
    /// Automatically open the browser after creating the merge request
    #[clap(long)]
    pub browse: bool,
    /// Open the merge request automatically without prompting for confirmation
    #[clap(long, short)]
    pub yes: bool,
    /// Adds and commits all changes before creating the merge request
    #[clap(long)]
    pub commit: Option<String>,
    /// Set up the merge request as draft
    #[clap(long)]
    pub draft: bool,
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
pub struct ListMergeRequest {
    #[clap()]
    pub state: MergeRequestStateStateCli,
    #[command(flatten)]
    pub list_args: ListArgs,
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

impl From<ListMergeRequest> for MergeRequestOptions {
    fn from(options: ListMergeRequest) -> Self {
        let list_args = gen_list_args(options.list_args);
        MergeRequestOptions::List(MergeRequestListCliArgs::new(
            options.state.into(),
            list_args,
        ))
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

impl From<CreateMergeRequest> for MergeRequestOptions {
    fn from(options: CreateMergeRequest) -> Self {
        MergeRequestOptions::Create(
            MergeRequestCliArgs::builder()
                .title(options.title)
                .title_from_commit(options.title_from_commit)
                .description(options.description)
                .description_from_file(options.description_from_file)
                .target_branch(options.target_branch)
                .auto(options.auto)
                .refresh_cache(options.refresh)
                .open_browser(options.browse)
                .accept_summary(options.yes)
                .commit(options.commit)
                .draft(options.draft)
                .build()
                .unwrap(),
        )
    }
}

pub enum MergeRequestOptions {
    Create(MergeRequestCliArgs),
    List(MergeRequestListCliArgs),
    Merge { id: i64 },
    Checkout { id: i64 },
    Close { id: i64 },
}
