use std::option::Option;

use clap::{Parser, ValueEnum};

use crate::cmds::merge_request::{
    CommentMergeRequestCliArgs, CommentMergeRequestListCliArgs, MergeRequestCliArgs,
    MergeRequestGetCliArgs, MergeRequestListCliArgs, MergeRequestState, SummaryOptions,
};

use super::common::{validate_project_repo_path, CacheArgs, GetArgs, ListArgs};

#[derive(Parser)]
pub struct MergeRequestCommand {
    #[clap(subcommand)]
    subcommand: MergeRequestSubcommand,
}

#[derive(Parser)]
enum MergeRequestSubcommand {
    #[clap(about = "Creates a merge request", visible_alias = "cr")]
    Create(CreateMergeRequest),
    #[clap(about = "Approve a merge request", visible_alias = "ap")]
    Approve(ApproveMergeRequest),
    #[clap(about = "Merge a merge request")]
    Merge(MergeMergeRequest),
    #[clap(about = "Git checkout a merge request branch for review")]
    Checkout(CheckoutMergeRequest),
    #[clap(
        subcommand,
        about = "Merge request comment operations",
        visible_alias = "cm"
    )]
    Comment(CommentSubCommand),
    #[clap(about = "Close a merge request")]
    Close(CloseMergeRequest),
    /// Get a merge request
    Get(GetMergeRequest),
    #[clap(about = "List merge requests", visible_alias = "ls")]
    List(ListMergeRequest),
}

#[derive(Parser)]
struct GetMergeRequest {
    /// Id of the merge request
    #[clap()]
    id: i64,
    #[clap(flatten)]
    get_args: GetArgs,
}

#[derive(Parser)]
enum CommentSubCommand {
    /// Create a comment to a given merge request
    Create(CreateCommentMergeRequest),
    /// List comments of a given merge request
    List(ListCommentMergeRequest),
}

#[derive(Parser)]
struct CreateCommentMergeRequest {
    /// Id of the merge request
    #[clap(long)]
    pub id: i64,
    /// Comment to add to the merge request
    #[clap(group = "comment_msg")]
    pub comment: Option<String>,
    /// Gather comment from the specified file. If "-" is provided, read from STDIN
    #[clap(long, value_name = "FILE", group = "comment_msg")]
    pub comment_from_file: Option<String>,
}

#[derive(Parser)]
struct ListCommentMergeRequest {
    /// Id of the merge request
    #[clap()]
    pub id: i64,
    #[command(flatten)]
    pub list_args: ListArgs,
}

#[derive(Clone, Debug, Parser, ValueEnum)]
enum SummaryCliOptions {
    Short,
    Long,
}

impl From<Option<SummaryCliOptions>> for SummaryOptions {
    fn from(options: Option<SummaryCliOptions>) -> Self {
        match options {
            Some(SummaryCliOptions::Short) => SummaryOptions::Short,
            Some(SummaryCliOptions::Long) => SummaryOptions::Long,
            None => SummaryOptions::None,
        }
    }
}

#[derive(Parser)]
struct CreateMergeRequest {
    /// Title of the merge request
    #[clap(long, group = "title_input")]
    pub title: Option<String>,
    /// Gather title and description from the specified commit message
    #[clap(
        long,
        group = "title_input",
        group = "description_input",
        value_name = "SHA"
    )]
    pub body_from_commit: Option<String>,
    /// Gather merge request title and description from the specified file. If "-" is
    /// provided, read from STDIN. Title and description are separated by a blank line.
    #[clap(
        long,
        group = "title_input",
        group = "description_input",
        value_name = "FILE"
    )]
    pub body_from_file: Option<String>,
    /// Description of the merge request
    #[clap(long, group = "description_input")]
    pub description: Option<String>,
    /// Gather merge request description from the specified file. If "-" is
    /// provided, read from STDIN
    #[clap(long, group = "description_input", value_name = "FILE")]
    pub description_from_file: Option<String>,
    /// Assignee username
    #[clap(long, short = 'A', value_name = "USERNAME")]
    pub assignee: Option<String>,
    /// Reviewer username
    #[clap(long, short = 'R', value_name = "USERNAME")]
    pub reviewer: Option<String>,
    /// Provides a list of outgoing commit SHAs and messages with subject
    /// (short) and body (long) to STDOUT, then exits. No merge request is created.
    #[clap(short, long, group = "summary_args", value_name = "OPTION")]
    pub summary: Option<SummaryCliOptions>,
    /// Provides a patch/diff of the outgoing changes to STDOUT, then exits. No merge
    /// request is created.
    #[clap(short, long, group = "summary_args")]
    pub patch: bool,
    /// Accept the default title, description, and target branch
    #[clap(long, short)]
    pub auto: bool,
    /// Provide a GPT prompt with the summary of the outgoing changes. This can
    /// be used to automatically create a title and a description for the
    /// outgoing merge request. Requires `--summary short` or `--summary long`.
    #[clap(long, short, requires = "summary")]
    pub gpt_prompt: bool,
    /// Automatically fetch the latest changes from the remote repository
    #[clap(long, value_name = "REMOTE_ALIAS")]
    pub fetch: Option<String>,
    /// Automatically rebase the current branch on top of the target branch
    #[clap(long, value_name = "REMOTE_ALIAS/BRANCH")]
    pub rebase: Option<String>,
    /// Open merge request in another `OWNER/PROJECT_NAME` instead of current
    /// origin.
    #[clap(long, value_name = "OWNER/PROJECT_NAME", value_parser=validate_project_repo_path, requires = "target_branch")]
    pub target_repo: Option<String>,
    /// Target branch of the merge request instead of default project's upstream
    /// branch. If targetting another repository, target branch is required.
    #[clap(long)]
    pub target_branch: Option<String>,
    /// Automatically open the browser after creating the merge request
    #[clap(long, short)]
    pub browse: bool,
    /// Open the merge request automatically without prompting for confirmation
    #[clap(long, short)]
    pub yes: bool,
    /// Adds and commits all changes before creating the merge request
    #[clap(long, value_name = "COMMIT_MSG")]
    pub commit: Option<String>,
    /// Update the merge request title and description with latest summary
    #[clap(long)]
    pub amend: bool,
    /// Force push the current branch to the remote repository
    #[clap(long, short)]
    pub force: bool,
    /// Set up the merge request as draft
    #[clap(long, visible_alias = "wip")]
    pub draft: bool,
    /// Dry run. Does not push the branch and does not create the merge request
    #[clap(long)]
    pub dry_run: bool,
    #[clap(flatten)]
    pub cache_args: CacheArgs,
}

#[derive(ValueEnum, Clone, PartialEq, Debug)]
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

#[derive(Parser)]
struct ApproveMergeRequest {
    /// Id of the merge request
    #[clap()]
    pub id: i64,
}

impl From<ListMergeRequest> for MergeRequestOptions {
    fn from(options: ListMergeRequest) -> Self {
        MergeRequestOptions::List(MergeRequestListCliArgs::new(
            options.state.into(),
            options.list_args.into(),
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

impl From<ApproveMergeRequest> for MergeRequestOptions {
    fn from(options: ApproveMergeRequest) -> Self {
        MergeRequestOptions::Approve { id: options.id }
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
            MergeRequestSubcommand::Comment(options) => options.into(),
            MergeRequestSubcommand::Get(options) => options.into(),
            MergeRequestSubcommand::Approve(options) => options.into(),
        }
    }
}

impl From<CommentSubCommand> for MergeRequestOptions {
    fn from(options: CommentSubCommand) -> Self {
        match options {
            CommentSubCommand::Create(options) => options.into(),
            CommentSubCommand::List(options) => options.into(),
        }
    }
}

impl From<CreateMergeRequest> for MergeRequestOptions {
    fn from(options: CreateMergeRequest) -> Self {
        MergeRequestOptions::Create(
            MergeRequestCliArgs::builder()
                .title(options.title)
                .body_from_commit(options.body_from_commit)
                .body_from_file(options.body_from_file)
                .description(options.description)
                .description_from_file(options.description_from_file)
                .assignee(options.assignee)
                .reviewer(options.reviewer)
                .target_branch(options.target_branch)
                .target_repo(options.target_repo)
                .fetch(options.fetch)
                .rebase(options.rebase)
                .auto(options.auto)
                .cache_args(options.cache_args.into())
                .open_browser(options.browse)
                .accept_summary(options.yes)
                .commit(options.commit)
                .draft(options.draft)
                .amend(options.amend)
                .force(options.force)
                .dry_run(options.dry_run)
                .summary(options.summary.into())
                .patch(options.patch)
                .gpt_prompt(options.gpt_prompt)
                .build()
                .unwrap(),
        )
    }
}

impl From<ListCommentMergeRequest> for MergeRequestOptions {
    fn from(options: ListCommentMergeRequest) -> Self {
        MergeRequestOptions::ListComment(
            CommentMergeRequestListCliArgs::builder()
                .id(options.id)
                .list_args(options.list_args.into())
                .build()
                .unwrap(),
        )
    }
}

impl From<CreateCommentMergeRequest> for MergeRequestOptions {
    fn from(options: CreateCommentMergeRequest) -> Self {
        MergeRequestOptions::CreateComment(
            CommentMergeRequestCliArgs::builder()
                .id(options.id)
                .comment(options.comment)
                .comment_from_file(options.comment_from_file)
                .build()
                .unwrap(),
        )
    }
}

impl From<GetMergeRequest> for MergeRequestOptions {
    fn from(options: GetMergeRequest) -> Self {
        MergeRequestOptions::Get(
            MergeRequestGetCliArgs::builder()
                .id(options.id)
                .get_args(options.get_args.into())
                .build()
                .unwrap(),
        )
    }
}

pub enum MergeRequestOptions {
    Create(MergeRequestCliArgs),
    Get(MergeRequestGetCliArgs),
    List(MergeRequestListCliArgs),
    CreateComment(CommentMergeRequestCliArgs),
    ListComment(CommentMergeRequestListCliArgs),
    Approve { id: i64 },
    Merge { id: i64 },
    // TODO: Checkout is a read operation, so we should propagate MergeRequestGetCliArgs
    Checkout { id: i64 },
    Close { id: i64 },
}

#[cfg(test)]
mod test {
    use crate::cli::{Args, Command};

    use super::*;

    #[test]
    fn test_list_merge_requests_cli_args() {
        let args = Args::parse_from(vec!["gr", "mr", "list", "opened"]);
        let list_merge_request = match args.command {
            Command::MergeRequest(MergeRequestCommand {
                subcommand: MergeRequestSubcommand::List(options),
            }) => {
                assert_eq!(options.state, MergeRequestStateStateCli::Opened);
                options
            }
            _ => panic!("Expected MergeRequestCommand::List"),
        };

        let options: MergeRequestOptions = list_merge_request.into();
        match options {
            MergeRequestOptions::List(args) => {
                assert_eq!(args.state, MergeRequestState::Opened);
            }
            _ => panic!("Expected MergeRequestOptions::List"),
        }
    }

    #[test]
    fn test_merge_merge_request_cli_args() {
        let args = Args::parse_from(vec!["gr", "mr", "merge", "123"]);
        let merge_merge_request = match args.command {
            Command::MergeRequest(MergeRequestCommand {
                subcommand: MergeRequestSubcommand::Merge(options),
            }) => {
                assert_eq!(options.id, 123);
                options
            }
            _ => panic!("Expected MergeRequestCommand::Merge"),
        };

        let options: MergeRequestOptions = merge_merge_request.into();
        match options {
            MergeRequestOptions::Merge { id } => {
                assert_eq!(id, 123);
            }
            _ => panic!("Expected MergeRequestOptions::Merge"),
        }
    }

    #[test]
    fn test_checkout_merge_request_cli_args() {
        let args = Args::parse_from(vec!["gr", "mr", "checkout", "123"]);
        let checkout_merge_request = match args.command {
            Command::MergeRequest(MergeRequestCommand {
                subcommand: MergeRequestSubcommand::Checkout(options),
            }) => {
                assert_eq!(options.id, 123);
                options
            }
            _ => panic!("Expected MergeRequestCommand::Checkout"),
        };

        let options: MergeRequestOptions = checkout_merge_request.into();
        match options {
            MergeRequestOptions::Checkout { id } => {
                assert_eq!(id, 123);
            }
            _ => panic!("Expected MergeRequestOptions::Checkout"),
        }
    }

    #[test]
    fn test_close_merge_request_cli_args() {
        let args = Args::parse_from(vec!["gr", "mr", "close", "123"]);
        let close_merge_request = match args.command {
            Command::MergeRequest(MergeRequestCommand {
                subcommand: MergeRequestSubcommand::Close(options),
            }) => {
                assert_eq!(options.id, 123);
                options
            }
            _ => panic!("Expected MergeRequestCommand::Close"),
        };

        let options: MergeRequestOptions = close_merge_request.into();
        match options {
            MergeRequestOptions::Close { id } => {
                assert_eq!(id, 123);
            }
            _ => panic!("Expected MergeRequestOptions::Close"),
        }
    }

    #[test]
    fn test_comment_merge_request_cli_args() {
        let args = Args::parse_from(vec!["gr", "mr", "comment", "create", "--id", "123", "LGTM"]);
        let comment_merge_request = match args.command {
            Command::MergeRequest(MergeRequestCommand {
                subcommand: MergeRequestSubcommand::Comment(options),
            }) => match options {
                CommentSubCommand::Create(args) => {
                    assert_eq!(args.id, 123);
                    assert_eq!(args.comment, Some("LGTM".to_string()));
                    args
                }
                _ => panic!("Expected CommentSubCommand::Create"),
            },
            _ => panic!("Expected MergeRequestCommand::Comment"),
        };

        let options: MergeRequestOptions = comment_merge_request.into();
        match options {
            MergeRequestOptions::CreateComment(args) => {
                assert_eq!(args.id, 123);
                assert_eq!(args.comment, Some("LGTM".to_string()));
            }
            _ => panic!("Expected MergeRequestOptions::Comment"),
        }
    }

    #[test]
    fn test_list_all_comments_in_merge_request_cli_args() {
        let args = Args::parse_from(vec!["gr", "mr", "comment", "list", "123"]);
        let list_comment_merge_request = match args.command {
            Command::MergeRequest(MergeRequestCommand {
                subcommand: MergeRequestSubcommand::Comment(options),
            }) => match options {
                CommentSubCommand::List(args) => {
                    assert_eq!(args.id, 123);
                    args
                }
                _ => panic!("Expected CommentSubCommand::List"),
            },
            _ => panic!("Expected MergeRequestCommand::Comment"),
        };

        let options: MergeRequestOptions = list_comment_merge_request.into();
        match options {
            MergeRequestOptions::ListComment(args) => {
                assert_eq!(args.id, 123);
            }
            _ => panic!("Expected MergeRequestOptions::ListComment"),
        }
    }

    #[test]
    fn test_create_merge_request_cli_args() {
        let args = Args::parse_from(vec!["gr", "mr", "create", "--auto", "-y", "--browse"]);
        let create_merge_request = match args.command {
            Command::MergeRequest(MergeRequestCommand {
                subcommand: MergeRequestSubcommand::Create(options),
            }) => {
                assert!(options.auto);
                assert!(options.yes);
                assert!(options.browse);
                options
            }
            _ => panic!("Expected MergeRequestCommand::Create"),
        };

        let options: MergeRequestOptions = create_merge_request.into();
        match options {
            MergeRequestOptions::Create(args) => {
                assert!(args.auto);
                assert!(args.accept_summary);
                assert!(args.open_browser);
            }
            _ => panic!("Expected MergeRequestOptions::Create"),
        }
    }

    #[test]
    fn test_get_merge_request_details_cli_args() {
        let args = Args::parse_from(vec!["gr", "mr", "get", "123"]);
        let get_merge_request = match args.command {
            Command::MergeRequest(MergeRequestCommand {
                subcommand: MergeRequestSubcommand::Get(options),
            }) => {
                assert_eq!(options.id, 123);
                options
            }
            _ => panic!("Expected MergeRequestCommand::Get"),
        };

        let options: MergeRequestOptions = get_merge_request.into();
        match options {
            MergeRequestOptions::Get(args) => {
                assert_eq!(args.id, 123);
            }
            _ => panic!("Expected MergeRequestOptions::Get"),
        }
    }

    #[test]
    fn test_wip_alias_as_draft() {
        let args = Args::parse_from(vec!["gr", "mr", "create", "--auto", "--wip"]);
        let create_merge_request = match args.command {
            Command::MergeRequest(MergeRequestCommand {
                subcommand: MergeRequestSubcommand::Create(options),
            }) => {
                assert!(options.draft);
                options
            }
            _ => panic!("Expected MergeRequestCommand::Create"),
        };

        let options: MergeRequestOptions = create_merge_request.into();
        match options {
            MergeRequestOptions::Create(args) => {
                assert!(args.draft);
            }
            _ => panic!("Expected MergeRequestOptions::Create"),
        }
    }

    #[test]
    fn test_title_description_cli_combinations() {
        // Valid combinations
        assert!(Args::try_parse_from(["gr", "mr", "create", "--title", "test"]).is_ok());
        assert!(Args::try_parse_from(["gr", "mr", "create", "--description", "test"]).is_ok());
        assert!(Args::try_parse_from([
            "gr",
            "mr",
            "create",
            "--title",
            "test",
            "--description",
            "test"
        ])
        .is_ok());
        assert!(Args::try_parse_from([
            "gr",
            "mr",
            "create",
            "--title",
            "test",
            "--description-from-file",
            "file.txt"
        ])
        .is_ok());
        assert!(
            Args::try_parse_from(["gr", "mr", "create", "--body-from-commit", "abc123"]).is_ok()
        );
        assert!(
            Args::try_parse_from(["gr", "mr", "create", "--body-from-file", "file.txt"]).is_ok()
        );

        // Invalid combinations
        assert!(Args::try_parse_from([
            "gr",
            "mr",
            "create",
            "--body-from-commit",
            "abc123",
            "--body-from-file",
            "file.txt"
        ])
        .is_err());

        assert!(Args::try_parse_from([
            "gr",
            "mr",
            "create",
            "--body-from-commit",
            "abc123",
            "--title",
            "test"
        ])
        .is_err());

        assert!(Args::try_parse_from([
            "gr",
            "mr",
            "create",
            "--body-from-file",
            "/tmp/file.txt",
            "--title",
            "test"
        ])
        .is_err());

        assert!(Args::try_parse_from([
            "gr",
            "mr",
            "create",
            "--body-from-file",
            "file.txt",
            "--description",
            "test"
        ])
        .is_err());

        assert!(Args::try_parse_from([
            "gr",
            "mr",
            "create",
            "--body-from-commit",
            "file.txt",
            "--description",
            "test"
        ])
        .is_err());

        assert!(Args::try_parse_from([
            "gr",
            "mr",
            "create",
            "--description",
            "test",
            "--description-from-file",
            "file.txt"
        ])
        .is_err());

        assert!(Args::try_parse_from([
            "gr",
            "mr",
            "create",
            "--body-from-file",
            "file.txt",
            "--description-from-file",
            "file.txt"
        ])
        .is_err());
    }
}
