use crate::{
    docker::{DockerImageCliArgs, DockerListCliArgs},
    merge_request::{MergeRequestCliArgs, MergeRequestListCliArgs},
    remote::{ListRemoteCliArgs, ListSortMode, MergeRequestState},
};

use std::{
    fmt::{self, Display, Formatter},
    option::Option,
};

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
    #[clap(name = "pj", about = "Gather project information metadata")]
    Project(ProjectCommand),
    #[clap(name = "init", about = "Initialize the config file")]
    Init(InitCommand),
    #[clap(
        name = "dk",
        about = "Handles docker images in Gitlab/Github registries"
    )]
    Docker(DockerCommand),
}

#[derive(Parser)]
struct DockerCommand {
    #[clap(subcommand)]
    pub subcommand: DockerSubCommand,
}

#[derive(Parser)]
enum DockerSubCommand {
    #[clap(about = "List Docker images")]
    List(ListDockerImages),
    #[clap(about = "Get docker image metadata")]
    Image(DockerImageMetadata),
}

#[derive(Parser)]
struct DockerImageMetadata {
    /// Repository ID the image belongs to
    #[clap(long)]
    repo_id: i64,
    /// Tag name
    #[clap()]
    tag: String,
}

#[derive(Parser)]
struct ListDockerImages {
    /// List image repositories in this projects' registry
    #[clap(long, default_value = "false", group = "list")]
    repos: bool,
    /// List all image tags for a given repository id
    #[clap(long, default_value = "false", group = "list", requires = "repo_id")]
    tags: bool,
    /// Repository ID to pull image tags from
    #[clap(long)]
    repo_id: Option<i64>,
    #[command(flatten)]
    list_args: ListArgs,
}

#[derive(Parser)]
struct InitCommand {
    #[clap(long)]
    pub domain: String,
}

#[derive(Parser)]
struct ProjectCommand {
    #[clap(subcommand)]
    pub subcommand: ProjectSubcommand,
    /// Refresh the cache
    #[clap(long, short)]
    pub refresh: bool,
}

#[derive(Parser)]
enum ProjectSubcommand {
    #[clap(about = "Gather project information metadata")]
    Info(ProjectInfo),
}

#[derive(Parser)]
struct ProjectInfo {
    /// ID of the project
    #[clap(long)]
    pub id: Option<i64>,
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
struct ListMergeRequest {
    #[clap()]
    pub state: MergeRequestStateStateCli,
    #[command(flatten)]
    list_args: ListArgs,
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
    pub subcommand: PipelineSubcommand,
}

#[derive(Parser)]
enum PipelineSubcommand {
    #[clap(about = "List pipelines")]
    List(ListArgs),
}

#[derive(Clone, Parser)]
struct ListArgs {
    /// From page
    #[clap(long)]
    from_page: Option<i64>,
    /// To page
    #[clap(long)]
    to_page: Option<i64>,
    /// How many pages are available
    #[clap(long)]
    num_pages: bool,
    /// Refresh the cache
    #[clap(long, short)]
    pub refresh: bool,
    /// Do not print headers
    #[clap(long)]
    no_headers: bool,
    /// List the given page number
    #[clap(long)]
    page: Option<i64>,
    /// Created after date (ISO 8601 YYYY-MM-DDTHH:MM:SSZ)
    #[clap(long)]
    created_after: Option<String>,
    /// Created before date (ISO 8601 YYYY-MM-DDTHH:MM:SSZ)
    #[clap(long)]
    created_before: Option<String>,
    #[clap(long, default_value_t=SortModeCli::Asc)]
    sort: SortModeCli,
}

#[derive(ValueEnum, Clone, Debug)]
enum SortModeCli {
    Asc,
    Desc,
}

impl Display for SortModeCli {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            SortModeCli::Asc => write!(f, "asc"),
            SortModeCli::Desc => write!(f, "desc"),
        }
    }
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
        Command::Project(sub_matches) => Some(CliOptions::Project(sub_matches.into())),
        Command::Init(sub_matches) => Some(CliOptions::Init(sub_matches.into())),
        Command::Docker(sub_matches) => Some(CliOptions::Docker(sub_matches.into())),
    }
}

pub enum CliOptions {
    MergeRequest(MergeRequestOptions),
    Browse(BrowseOptions),
    Pipeline(PipelineOptions),
    Project(ProjectOptions),
    Init(InitCommandOptions),
    Docker(DockerOptions),
}

pub struct InitCommandOptions {
    pub domain: String,
}

pub enum BrowseOptions {
    // defaults to open repo in browser
    Repo,
    MergeRequests,
    MergeRequestId(i64),
    Pipelines,
}

pub enum PipelineOptions {
    List(ListRemoteCliArgs),
}

#[derive(Debug)]
pub enum ProjectOperation {
    Info { id: Option<i64> },
}

#[derive(Debug)]
pub struct ProjectOptions {
    pub operation: ProjectOperation,
    pub refresh_cache: bool,
}

pub enum MergeRequestOptions {
    Create(MergeRequestCliArgs),
    List(MergeRequestListCliArgs),
    Merge { id: i64 },
    Checkout { id: i64 },
    Close { id: i64 },
}

pub enum DockerOptions {
    List(DockerListCliArgs),
    Get(DockerImageCliArgs),
}

// From impls - private clap structs to public domain structs
// Mainly to avoid propagating clap further down the stack as changes in the
// clap API could break other parts of the code.

impl From<DockerCommand> for DockerOptions {
    fn from(options: DockerCommand) -> Self {
        match options.subcommand {
            DockerSubCommand::List(options) => options.into(),
            DockerSubCommand::Image(options) => options.into(),
        }
    }
}

impl From<DockerImageMetadata> for DockerOptions {
    fn from(options: DockerImageMetadata) -> Self {
        DockerOptions::Get(
            DockerImageCliArgs::builder()
                .repo_id(options.repo_id)
                .tag(options.tag)
                .build()
                .unwrap(),
        )
    }
}

impl From<ListDockerImages> for DockerOptions {
    fn from(options: ListDockerImages) -> Self {
        let list_args = gen_list_args(options.list_args);
        DockerOptions::List(
            DockerListCliArgs::builder()
                .repos(options.repos)
                .tags(options.tags)
                .repo_id(options.repo_id)
                .list_args(list_args)
                .build()
                .unwrap(),
        )
    }
}

fn gen_list_args(list_args: ListArgs) -> ListRemoteCliArgs {
    let list_args = ListRemoteCliArgs::builder()
        .from_page(list_args.from_page)
        .to_page(list_args.to_page)
        .page_number(list_args.page)
        .num_pages(list_args.num_pages)
        .refresh_cache(list_args.refresh)
        .no_headers(list_args.no_headers)
        .created_after(list_args.created_after)
        .created_before(list_args.created_before)
        .sort(list_args.sort.into())
        .build()
        .unwrap();
    list_args
}

impl From<CreateMergeRequest> for MergeRequestOptions {
    fn from(options: CreateMergeRequest) -> Self {
        MergeRequestOptions::Create(
            MergeRequestCliArgs::builder()
                .title(options.title)
                .description(options.description)
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

impl From<SortModeCli> for ListSortMode {
    fn from(sort: SortModeCli) -> Self {
        match sort {
            SortModeCli::Asc => ListSortMode::Asc,
            SortModeCli::Desc => ListSortMode::Desc,
        }
    }
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
            PipelineSubcommand::List(options) => options.into(),
        }
    }
}

impl From<ListArgs> for PipelineOptions {
    fn from(options: ListArgs) -> Self {
        PipelineOptions::List(gen_list_args(options))
    }
}

impl From<ProjectCommand> for ProjectOptions {
    fn from(options: ProjectCommand) -> Self {
        match options.subcommand {
            ProjectSubcommand::Info(options_info) => ProjectOptions {
                operation: ProjectOperation::Info {
                    id: options_info.id,
                },
                refresh_cache: options.refresh,
            },
        }
    }
}

impl From<InitCommand> for InitCommandOptions {
    fn from(options: InitCommand) -> Self {
        InitCommandOptions {
            domain: options.domain,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_docker_cli_repos() {
        let args = Args::parse_from(vec!["gr", "dk", "list", "--repos"]);
        match args.command {
            Command::Docker(DockerCommand {
                subcommand: DockerSubCommand::List(options),
            }) => {
                assert_eq!(options.repos, true);
                assert_eq!(options.tags, false);
            }
            _ => panic!("Expected DockerCommand"),
        }
    }

    #[test]
    fn test_docker_cli_tags() {
        let args = Args::parse_from(vec!["gr", "dk", "list", "--tags", "--repo-id", "12"]);
        match args.command {
            Command::Docker(DockerCommand {
                subcommand: DockerSubCommand::List(options),
            }) => {
                assert_eq!(options.repos, false);
                assert_eq!(options.tags, true);
                assert_eq!(options.repo_id, Some(12));
            }
            _ => panic!("Expected DockerCommand"),
        }
    }
}
