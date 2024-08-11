use crate::api_traits::{CommentMergeRequest, MergeRequest, RemoteProject, Timestamp};
use crate::cli::merge_request::MergeRequestOptions;
use crate::cli::CliArgs;
use crate::config::ConfigProperties;
use crate::display::{Column, DisplayBody};
use crate::error::{AddContext, GRError};
use crate::git::Repo;
use crate::io::{CmdInfo, Response, TaskRunner};
use crate::remote::{CacheCliArgs, CacheType, GetRemoteCliArgs, ListBodyArgs, ListRemoteCliArgs};
use crate::shell::BlockingCommand;
use crate::{dialog, display, exec, git, remote, Cmd, Result};
use std::fmt::{self, Display, Formatter};
use std::{
    fs::File,
    io::{BufRead, BufReader, Cursor, Write},
    sync::Arc,
};

use super::common::{self, get_user};
use super::project::{Member, Project};

#[derive(Builder, Clone, Debug, Default)]
#[builder(default)]
pub struct MergeRequestResponse {
    pub id: i64,
    pub web_url: String,
    pub author: String,
    pub updated_at: String,
    pub source_branch: String,
    pub sha: String,
    pub created_at: String,
    pub title: String,
    // For Github to filter pull requests from issues.
    pub pull_request: String,
    // Optional fields to display for get and list operations
    pub description: String,
    pub merged_at: String,
    pub pipeline_id: Option<i64>,
    pub pipeline_url: Option<String>,
}

impl MergeRequestResponse {
    pub fn builder() -> MergeRequestResponseBuilder {
        MergeRequestResponseBuilder::default()
    }
}

impl From<MergeRequestResponse> for DisplayBody {
    fn from(mr: MergeRequestResponse) -> DisplayBody {
        DisplayBody {
            columns: vec![
                Column::new("ID", mr.id.to_string()),
                Column::new("Title", mr.title),
                Column::new("Source Branch", mr.source_branch),
                Column::builder()
                    .name("SHA".to_string())
                    .value(mr.sha)
                    .optional(true)
                    .build()
                    .unwrap(),
                Column::builder()
                    .name("Description".to_string())
                    .value(mr.description)
                    .optional(true)
                    .build()
                    .unwrap(),
                Column::new("Author", mr.author),
                Column::new("URL", mr.web_url),
                Column::new("Updated at", mr.updated_at),
                Column::builder()
                    .name("Merged at".to_string())
                    .value(mr.merged_at)
                    .optional(true)
                    .build()
                    .unwrap(),
                Column::builder()
                    .name("Pipeline ID".to_string())
                    .value(mr.pipeline_id.map_or("".to_string(), |id| id.to_string()))
                    .optional(true)
                    .build()
                    .unwrap(),
                Column::builder()
                    .name("Pipeline URL".to_string())
                    .value(mr.pipeline_url.unwrap_or("".to_string()))
                    .optional(true)
                    .build()
                    .unwrap(),
            ],
        }
    }
}

impl Timestamp for MergeRequestResponse {
    fn created_at(&self) -> String {
        self.created_at.clone()
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum MergeRequestState {
    Opened,
    Closed,
    Merged,
}

impl TryFrom<&str> for MergeRequestState {
    type Error = String;

    fn try_from(s: &str) -> std::result::Result<Self, Self::Error> {
        match s {
            "opened" => Ok(MergeRequestState::Opened),
            "closed" => Ok(MergeRequestState::Closed),
            "merged" => Ok(MergeRequestState::Merged),
            _ => Err(format!("Invalid merge request state: {}", s)),
        }
    }
}

impl Display for MergeRequestState {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            MergeRequestState::Opened => write!(f, "opened"),
            MergeRequestState::Closed => write!(f, "closed"),
            MergeRequestState::Merged => write!(f, "merged"),
        }
    }
}

#[derive(Builder)]
pub struct MergeRequestBodyArgs {
    #[builder(default)]
    pub title: String,
    #[builder(default)]
    pub description: String,
    #[builder(default)]
    pub source_branch: String,
    #[builder(default)]
    pub target_repo: String,
    #[builder(default)]
    pub target_branch: String,
    #[builder(default)]
    pub assignee_id: String,
    #[builder(default)]
    pub username: String,
    #[builder(default = "String::from(\"true\")")]
    pub remove_source_branch: String,
    #[builder(default)]
    pub draft: bool,
    #[builder(default)]
    pub amend: bool,
}

impl MergeRequestBodyArgs {
    pub fn builder() -> MergeRequestBodyArgsBuilder {
        MergeRequestBodyArgsBuilder::default()
    }
}

#[derive(Builder, Clone)]
pub struct MergeRequestListBodyArgs {
    pub state: MergeRequestState,
    pub list_args: Option<ListBodyArgs>,
    #[builder(default)]
    pub assignee: Option<Member>,
    #[builder(default)]
    pub author: Option<Member>,
    #[builder(default)]
    pub reviewer: Option<Member>,
}

impl MergeRequestListBodyArgs {
    pub fn builder() -> MergeRequestListBodyArgsBuilder {
        MergeRequestListBodyArgsBuilder::default()
    }
}

#[derive(Builder, Clone)]
pub struct MergeRequestCliArgs {
    pub title: Option<String>,
    pub title_from_commit: Option<String>,
    pub description: Option<String>,
    pub description_from_file: Option<String>,
    pub target_branch: Option<String>,
    #[builder(default)]
    pub target_repo: Option<String>,
    #[builder(default)]
    pub fetch: Option<String>,
    #[builder(default)]
    pub rebase: Option<String>,
    pub auto: bool,
    pub cache_args: CacheCliArgs,
    pub open_browser: bool,
    pub accept_summary: bool,
    pub commit: Option<String>,
    pub amend: bool,
    pub force: bool,
    pub draft: bool,
}

impl MergeRequestCliArgs {
    pub fn builder() -> MergeRequestCliArgsBuilder {
        MergeRequestCliArgsBuilder::default()
    }
}

/// Enum for filtering merge requests by user
/// Me: current authenticated user
/// Other: another username, provided by cli flags.
#[derive(Clone, Debug, PartialEq)]
pub enum MergeRequestUser {
    Me,
    Other(String),
}

#[derive(Builder)]
pub struct MergeRequestListCliArgs {
    pub state: MergeRequestState,
    pub list_args: ListRemoteCliArgs,
    // Filtering options. Make use of the builder pattern.
    #[builder(default)]
    pub assignee: Option<MergeRequestUser>,
    #[builder(default)]
    pub author: Option<MergeRequestUser>,
    #[builder(default)]
    pub reviewer: Option<MergeRequestUser>,
}

impl MergeRequestListCliArgs {
    pub fn new(state: MergeRequestState, args: ListRemoteCliArgs) -> MergeRequestListCliArgs {
        MergeRequestListCliArgs {
            state,
            list_args: args,
            assignee: None,
            author: None,
            reviewer: None,
        }
    }
    pub fn builder() -> MergeRequestListCliArgsBuilder {
        MergeRequestListCliArgsBuilder::default()
    }
}

#[derive(Builder)]
pub struct MergeRequestGetCliArgs {
    pub id: i64,
    pub get_args: GetRemoteCliArgs,
}

impl MergeRequestGetCliArgs {
    pub fn builder() -> MergeRequestGetCliArgsBuilder {
        MergeRequestGetCliArgsBuilder::default()
    }
}

#[derive(Builder)]
pub struct CommentMergeRequestCliArgs {
    pub id: i64,
    pub comment: Option<String>,
    pub comment_from_file: Option<String>,
}

impl CommentMergeRequestCliArgs {
    pub fn builder() -> CommentMergeRequestCliArgsBuilder {
        CommentMergeRequestCliArgsBuilder::default()
    }
}

#[derive(Builder)]
pub struct CommentMergeRequestListCliArgs {
    pub id: i64,
    pub list_args: ListRemoteCliArgs,
}

impl CommentMergeRequestListCliArgs {
    pub fn builder() -> CommentMergeRequestListCliArgsBuilder {
        CommentMergeRequestListCliArgsBuilder::default()
    }
}

#[derive(Builder)]
pub struct CommentMergeRequestListBodyArgs {
    pub id: i64,
    pub list_args: Option<ListBodyArgs>,
}

impl CommentMergeRequestListBodyArgs {
    pub fn builder() -> CommentMergeRequestListBodyArgsBuilder {
        CommentMergeRequestListBodyArgsBuilder::default()
    }
}

#[derive(Builder)]
pub struct CommentMergeRequestBodyArgs {
    pub id: i64,
    pub comment: String,
}

impl CommentMergeRequestBodyArgs {
    pub fn builder() -> CommentMergeRequestBodyArgsBuilder {
        CommentMergeRequestBodyArgsBuilder::default()
    }
}

#[derive(Builder, Clone)]
pub struct Comment {
    pub id: i64,
    pub body: String,
    pub author: String,
    pub created_at: String,
}

impl Comment {
    pub fn builder() -> CommentBuilder {
        CommentBuilder::default()
    }
}

impl Timestamp for Comment {
    fn created_at(&self) -> String {
        self.created_at.clone()
    }
}

impl From<Comment> for DisplayBody {
    fn from(comment: Comment) -> Self {
        DisplayBody::new(vec![
            Column::new("ID", comment.id.to_string()),
            Column::new("Body", comment.body),
            Column::new("Author", comment.author),
            Column::new("Created at", comment.created_at),
        ])
    }
}

pub fn execute(
    options: MergeRequestOptions,
    global_args: CliArgs,
    config: Arc<dyn ConfigProperties>,
    domain: String,
    path: String,
) -> Result<()> {
    match options {
        MergeRequestOptions::Create(cli_args) => {
            if global_args.repo.is_some() {
                return Err(GRError::PreconditionNotMet(
                    "--repo not allowed when creating a merge request".to_string(),
                )
                .into());
            }
            let mr_remote = remote::get_mr(
                domain.clone(),
                path.clone(),
                config.clone(),
                Some(&cli_args.cache_args),
                CacheType::File,
            )?;
            let project_remote = remote::get_project(
                domain,
                path,
                config.clone(),
                Some(&cli_args.cache_args),
                CacheType::File,
            )?;
            if let Some(commit_message) = &cli_args.commit {
                git::add(&BlockingCommand)?;
                git::commit(&BlockingCommand, commit_message)?;
            }
            let cmds = if let Some(description_file) = &cli_args.description_from_file {
                let reader = get_reader_file_cli(description_file)?;
                cmds(
                    project_remote,
                    &cli_args,
                    Arc::new(BlockingCommand),
                    Some(reader),
                )
            } else {
                cmds(
                    project_remote,
                    &cli_args,
                    Arc::new(BlockingCommand),
                    None::<Cursor<&str>>,
                )
            };
            let mr_body = get_repo_project_info(cmds)?;
            open(mr_remote, config, mr_body, &cli_args)
        }
        MergeRequestOptions::List(cli_args) => list_merge_requests(domain, path, config, cli_args),
        MergeRequestOptions::Merge { id } => {
            let remote = remote::get_mr(domain, path, config, None, CacheType::None)?;
            merge(remote, id)
        }
        MergeRequestOptions::Checkout { id } => {
            // TODO: It should propagate the cache cli args.
            let remote = remote::get_mr(domain, path, config, None, CacheType::File)?;
            checkout(remote, id)
        }
        MergeRequestOptions::Close { id } => {
            let remote = remote::get_mr(domain, path, config, None, CacheType::None)?;
            close(remote, id)
        }
        MergeRequestOptions::CreateComment(cli_args) => {
            let remote = remote::get_comment_mr(domain, path, config, None, CacheType::None)?;
            if let Some(comment_file) = &cli_args.comment_from_file {
                let reader = get_reader_file_cli(comment_file)?;
                create_comment(remote, cli_args, Some(reader))
            } else {
                create_comment(remote, cli_args, None::<Cursor<&str>>)
            }
        }
        MergeRequestOptions::ListComment(cli_args) => {
            let remote = remote::get_comment_mr(
                domain,
                path,
                config,
                Some(&cli_args.list_args.get_args.cache_args),
                CacheType::File,
            )?;
            let from_to_args = remote::validate_from_to_page(&cli_args.list_args)?;
            let body_args = CommentMergeRequestListBodyArgs::builder()
                .id(cli_args.id)
                .list_args(from_to_args)
                .build()?;
            if cli_args.list_args.num_pages {
                return common::num_comment_merge_request_pages(
                    remote,
                    body_args,
                    std::io::stdout(),
                );
            }
            if cli_args.list_args.num_resources {
                return common::num_comment_merge_request_resources(
                    remote,
                    body_args,
                    std::io::stdout(),
                );
            }
            list_comments(remote, body_args, cli_args, std::io::stdout())
        }
        MergeRequestOptions::Get(cli_args) => {
            let remote = remote::get_mr(
                domain,
                path,
                config,
                Some(&cli_args.get_args.cache_args),
                CacheType::File,
            )?;
            get_merge_request_details(remote, cli_args, std::io::stdout())
        }
        MergeRequestOptions::Approve { id } => {
            let remote = remote::get_mr(domain, path, config, None, CacheType::None)?;
            approve(remote, id, std::io::stdout())
        }
    }
}

pub fn get_reader_file_cli(file_path: &str) -> Result<Box<dyn BufRead + Send + Sync>> {
    if file_path == "-" {
        Ok(Box::new(BufReader::new(std::io::stdin())))
    } else {
        let file = File::open(file_path).err_context(GRError::PreconditionNotMet(format!(
            "Cannot open file {}",
            file_path
        )))?;
        Ok(Box::new(BufReader::new(file)))
    }
}

fn get_filter_user(
    user: &Option<MergeRequestUser>,
    domain: &str,
    path: &str,
    config: &Arc<dyn ConfigProperties>,
    list_args: &ListRemoteCliArgs,
) -> Result<Option<Member>> {
    let member = match user {
        Some(MergeRequestUser::Me) => Some(get_user(domain, path, config, list_args)?),
        // TODO filter by specific username, not necessarily the
        // authenticated user.
        _ => None,
    };
    Ok(member)
}

pub fn list_merge_requests(
    domain: String,
    path: String,
    config: Arc<dyn ConfigProperties>,
    cli_args: MergeRequestListCliArgs,
) -> Result<()> {
    // Author, assignee and reviewer are mutually exclusive filters checked on
    // cli's flags. While we do sequential calls to retrieve them it is a very
    // fast operation. Only one ends up calling the remote to retrieve it's id.
    let author = get_filter_user(
        &cli_args.author,
        &domain,
        &path,
        &config,
        &cli_args.list_args,
    )?;

    let assignee = get_filter_user(
        &cli_args.assignee,
        &domain,
        &path,
        &config,
        &cli_args.list_args,
    )?;

    let reviewer = get_filter_user(
        &cli_args.reviewer,
        &domain,
        &path,
        &config,
        &cli_args.list_args,
    )?;

    let remote = remote::get_mr(
        domain,
        path,
        config,
        Some(&cli_args.list_args.get_args.cache_args),
        CacheType::File,
    )?;

    let from_to_args = remote::validate_from_to_page(&cli_args.list_args)?;
    let body_args = MergeRequestListBodyArgs::builder()
        .list_args(from_to_args)
        .state(cli_args.state)
        .assignee(assignee)
        .author(author)
        .reviewer(reviewer)
        .build()?;
    if cli_args.list_args.num_pages {
        return common::num_merge_request_pages(remote, body_args, std::io::stdout());
    }
    if cli_args.list_args.num_resources {
        return common::num_merge_request_resources(remote, body_args, std::io::stdout());
    }
    list(remote, body_args, cli_args, std::io::stdout())
}

fn user_prompt_confirmation(
    mr_body: &MergeRequestBody,
    config: Arc<dyn ConfigProperties>,
    description: String,
    target_branch: &String,
    cli_args: &MergeRequestCliArgs,
) -> Result<MergeRequestBodyArgs> {
    let mut title = mr_body.repo.title().to_string();
    if cli_args.draft {
        title = format!("DRAFT: {}", title);
    }
    if cli_args.target_repo.is_some() {
        // Targetting another repo different than the origin. Bypass gathering
        // of assignee members and prompt user for title and description only.
        let mut description = description;
        if !cli_args.auto {
            (title, description) = dialog::prompt_user_title_description(&title, &description);
        }
        return Ok(MergeRequestBodyArgs::builder()
            .title(title)
            .description(description)
            .source_branch(mr_body.repo.current_branch().to_string())
            .target_branch(target_branch.to_string())
            .target_repo(cli_args.target_repo.as_ref().unwrap().clone())
            .assignee_id("".to_string())
            .username("".to_string())
            .remove_source_branch("true".to_string())
            .amend(cli_args.amend)
            .draft(cli_args.draft)
            .build()?);
    }
    let user_input = if cli_args.auto {
        let preferred_assignee_members = mr_body
            .members
            .iter()
            .filter(|member| member.username == config.preferred_assignee_username())
            .collect::<Vec<&Member>>();
        if preferred_assignee_members.len() != 1 {
            return Err(GRError::PreconditionNotMet(
                "Cannot get preferred assignee user id".to_string(),
            )
            .into());
        }
        dialog::MergeRequestUserInput::new(
            &title,
            &description,
            preferred_assignee_members[0].id,
            &preferred_assignee_members[0].username,
        )
    } else {
        dialog::prompt_user_merge_request_info(&title, &description, &mr_body.members, config)?
    };

    Ok(MergeRequestBodyArgs::builder()
        .title(user_input.title)
        .description(user_input.description)
        .source_branch(mr_body.repo.current_branch().to_string())
        .target_branch(target_branch.to_string())
        .assignee_id(user_input.user_id.to_string())
        .username(user_input.username)
        // TODO make this configurable
        .remove_source_branch("true".to_string())
        .draft(cli_args.draft)
        .amend(cli_args.amend)
        .build()?)
}

/// Open a merge request.
fn open(
    remote: Arc<dyn MergeRequest>,
    config: Arc<dyn ConfigProperties>,
    mr_body: MergeRequestBody,
    cli_args: &MergeRequestCliArgs,
) -> Result<()> {
    let source_branch = &mr_body.repo.current_branch();
    let target_branch = cli_args.target_branch.clone();
    let target_branch = target_branch.unwrap_or(mr_body.project.default_branch().to_string());

    let description = build_description(
        mr_body.repo.last_commit_message(),
        config.merge_request_description_signature(),
    );

    // make sure we are in a feature branch or bail
    in_feature_branch(source_branch, &target_branch)?;

    // confirm title, description and assignee
    let args = user_prompt_confirmation(&mr_body, config, description, &target_branch, cli_args)?;

    if cli_args.rebase.is_some() {
        git::rebase(&BlockingCommand, cli_args.rebase.as_ref().unwrap())?;
    }

    let outgoing_commits = git::outgoing_commits(&BlockingCommand, "origin", &target_branch)?;

    if outgoing_commits.is_empty() {
        return Err(GRError::PreconditionNotMet(
            "No outgoing commits found. Please commit your changes.".to_string(),
        )
        .into());
    }

    // show summary of merge request and confirm
    if let Ok(()) =
        dialog::show_summary_merge_request(&outgoing_commits, &args, cli_args.accept_summary)
    {
        println!("\nTaking off... 🚀\n");
        git::push(&BlockingCommand, "origin", &mr_body.repo, cli_args.force)?;
        let merge_request_response = remote.open(args)?;
        println!("Merge request opened: {}", merge_request_response.web_url);
        if cli_args.open_browser {
            open::that(merge_request_response.web_url)?;
        }
    }
    Ok(())
}

/// Required commands to build a Project and a Repository
fn cmds<R: BufRead + Send + Sync + 'static>(
    remote: Arc<dyn RemoteProject + Send + Sync + 'static>,
    cli_args: &MergeRequestCliArgs,
    task_runner: Arc<impl TaskRunner<Response = Response> + Send + Sync + 'static>,
    reader: Option<R>,
) -> Vec<Cmd<CmdInfo>> {
    let remote_cl = remote.clone();
    let remote_project_cmd = move || -> Result<CmdInfo> { remote_cl.get_project_data(None, None) };
    let remote_members_cmd = move || -> Result<CmdInfo> { remote.get_project_members() };
    let status_runner = task_runner.clone();
    let git_status_cmd = || -> Result<CmdInfo> { git::status(status_runner) };
    let title = cli_args.title.clone();
    let title = title.unwrap_or("".to_string());
    let title_from_commit = cli_args.title_from_commit.clone();
    // if we are required to gather the title from specific commit, gather also
    // its description. The description will be pulled from the same commit as
    // the title.
    let description_commit = cli_args.title_from_commit.clone();
    let commit_summary_runner = task_runner.clone();
    let git_title_cmd = move || -> Result<CmdInfo> {
        if title.is_empty() {
            git::commit_summary(commit_summary_runner, &title_from_commit)
        } else {
            Ok(CmdInfo::CommitSummary(title.clone()))
        }
    };
    let current_branch_runner = task_runner.clone();
    let git_current_branch = || -> Result<CmdInfo> { git::current_branch(current_branch_runner) };
    let description = cli_args.description.clone();
    let description = description.unwrap_or("".to_string());
    let commit_msg_runner = task_runner.clone();
    let git_last_commit_message = move || -> Result<CmdInfo> {
        if description.is_empty() {
            if let Some(reader) = reader {
                let mut description = String::new();
                for line in reader.lines() {
                    let line = line?;
                    description.push_str(&line);
                    description.push('\n');
                }
                Ok(CmdInfo::CommitMessage(description))
            } else {
                git::commit_message(commit_msg_runner, &description_commit)
            }
        } else {
            Ok(CmdInfo::CommitMessage(description.clone()))
        }
    };
    let mut cmds: Vec<Cmd<CmdInfo>> = vec![
        Box::new(remote_project_cmd),
        Box::new(git_status_cmd),
        Box::new(git_title_cmd),
        Box::new(git_current_branch),
        Box::new(git_last_commit_message),
    ];
    // Only gather project members if we are not targeting a different repo
    if cli_args.target_repo.is_none() {
        cmds.push(Box::new(remote_members_cmd));
    }
    if cli_args.fetch.is_some() {
        let fetch_runner = task_runner.clone();
        let remote_alias = cli_args.fetch.as_ref().unwrap().clone();
        let git_fetch_cmd = || -> Result<CmdInfo> { git::fetch(fetch_runner, remote_alias) };
        cmds.push(Box::new(git_fetch_cmd));
    }
    cmds
}

// append description signature from the configuration
fn build_description(description: &str, signature: &str) -> String {
    if description.is_empty() && signature.is_empty() {
        return "".to_string();
    }
    if description.is_empty() {
        return signature.to_string();
    }
    if signature.is_empty() {
        return description.to_string();
    }
    format!("{}\n\n{}", description, signature)
}

#[derive(Builder)]
struct MergeRequestBody {
    repo: Repo,
    project: Project,
    members: Vec<Member>,
}

fn get_repo_project_info(cmds: Vec<Cmd<CmdInfo>>) -> Result<MergeRequestBody> {
    let mut project = Project::default();
    let mut members = Vec::new();
    let mut repo = git::Repo::default();
    let cmd_results = exec::parallel_stream(cmds);
    for cmd_result in cmd_results {
        match cmd_result {
            Ok(CmdInfo::Project(project_data)) => {
                project = project_data;
            }
            Ok(CmdInfo::Members(members_data)) => {
                members = members_data;
            }
            Ok(CmdInfo::StatusModified(status)) => repo.with_status(status),
            Ok(CmdInfo::Branch(branch)) => repo.with_branch(&branch),
            Ok(CmdInfo::CommitSummary(title)) => repo.with_title(&title),
            Ok(CmdInfo::CommitMessage(message)) => repo.with_last_commit_message(&message),
            // bail on first error found
            Err(e) => return Err(e),
            _ => {}
        }
    }
    Ok(MergeRequestBodyBuilder::default()
        .repo(repo)
        .project(project)
        .members(members)
        .build()?)
}

/// This makes sure we don't push to branches considered to be upstream in most cases.
fn in_feature_branch(current_branch: &str, upstream_branch: &str) -> Result<()> {
    if current_branch == upstream_branch {
        let trace = format!(
            "Current branch {} is the same as the upstream \
        remote {}. Please use a feature branch",
            current_branch, upstream_branch
        );
        return Err(GRError::PreconditionNotMet(trace).into());
    }
    // Being extra-careful. Avoid potential main, master, develop branches
    // also.
    match current_branch {
        "master" | "main" | "develop" => {
            let trace = format!(
                "Current branch is {}, which could be a release upstream branch. \
                Please use a different feature branch name",
                current_branch
            );
            Err(GRError::PreconditionNotMet(trace).into())
        }
        _ => Ok(()),
    }
}

fn list<W: Write>(
    remote: Arc<dyn MergeRequest>,
    body_args: MergeRequestListBodyArgs,
    cli_args: MergeRequestListCliArgs,
    mut writer: W,
) -> Result<()> {
    common::list_merge_requests(remote, body_args, cli_args, &mut writer)
}

fn merge(remote: Arc<dyn MergeRequest>, merge_request_id: i64) -> Result<()> {
    let merge_request = remote.merge(merge_request_id)?;
    println!("Merge request merged: {}", merge_request.web_url);
    Ok(())
}

fn checkout(remote: Arc<dyn MergeRequest>, id: i64) -> Result<()> {
    let merge_request = remote.get(id)?;
    // assume origin for now
    git::fetch(Arc::new(BlockingCommand), "origin".to_string())?;
    git::checkout(&BlockingCommand, &merge_request.source_branch)
}

fn close(remote: Arc<dyn MergeRequest>, id: i64) -> Result<()> {
    let merge_request = remote.close(id)?;
    println!("Merge request closed: {}", merge_request.web_url);
    Ok(())
}

fn approve<W: Write>(remote: Arc<dyn MergeRequest>, id: i64, mut writer: W) -> Result<()> {
    let merge_request = remote.approve(id)?;
    writer.write_all(format!("Merge request approved: {}\n", merge_request.web_url).as_bytes())?;
    Ok(())
}

fn create_comment<R: BufRead>(
    remote: Arc<dyn CommentMergeRequest>,
    args: CommentMergeRequestCliArgs,
    reader: Option<R>,
) -> Result<()> {
    let comment = if let Some(comment) = args.comment {
        comment
    } else {
        let mut comment = String::new();
        // The unwrap is Ok here. This is enforced at the CLI interface. The
        // user is required to provide a file or a comment.
        reader.unwrap().read_to_string(&mut comment)?;
        comment
    };
    remote.create(
        CommentMergeRequestBodyArgs::builder()
            .id(args.id)
            .comment(comment)
            .build()
            .unwrap(),
    )
}

pub fn get_merge_request_details<W: Write>(
    remote: Arc<dyn MergeRequest>,
    args: MergeRequestGetCliArgs,
    mut writer: W,
) -> Result<()> {
    let response = remote.get(args.id)?;
    display::print(&mut writer, vec![response], args.get_args)?;
    Ok(())
}

fn list_comments<W: Write>(
    remote: Arc<dyn CommentMergeRequest>,
    body_args: CommentMergeRequestListBodyArgs,
    cli_args: CommentMergeRequestListCliArgs,
    writer: W,
) -> Result<()> {
    common::list_merge_request_comments(remote, body_args, cli_args, writer)
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Cursor, Read},
        sync::Mutex,
    };

    use crate::{
        api_traits::CommentMergeRequest, cli::browse::BrowseOptions,
        cmds::project::ProjectListBodyArgs, error,
    };

    use super::*;

    #[test]
    fn test_merge_request_args_with_custom_title() {
        let args = MergeRequestBodyArgs::builder()
            .source_branch("source".to_string())
            .target_branch("target".to_string())
            .title("title".to_string())
            .build()
            .unwrap();

        assert_eq!(args.source_branch, "source");
        assert_eq!(args.target_branch, "target");
        assert_eq!(args.title, "title");
        assert_eq!(args.remove_source_branch, "true");
        assert_eq!(args.description, "");
    }

    #[test]
    fn test_merge_request_get_all_fields() {
        let args = MergeRequestBodyArgs::builder()
            .source_branch("source".to_string())
            .target_branch("target".to_string())
            .title("title".to_string())
            .description("description".to_string())
            .assignee_id("assignee_id".to_string())
            .username("username".to_string())
            .remove_source_branch("false".to_string())
            .build()
            .unwrap();

        assert_eq!(args.source_branch, "source");
        assert_eq!(args.target_branch, "target");
        assert_eq!(args.title, "title");
        assert_eq!(args.description, "description");
        assert_eq!(args.assignee_id, "assignee_id");
        assert_eq!(args.username, "username");
        assert_eq!(args.remove_source_branch, "false");
    }

    #[test]
    fn test_current_branch_should_not_be_the_upstream_branch() {
        let current_branch = "current-branch";
        let target_branch = "current-branch";
        let result = in_feature_branch(current_branch, target_branch);
        assert!(result.is_err());
    }

    #[test]
    fn test_feature_branch_not_main_master_or_develop_is_ok() {
        let current_branch = "newfeature";
        let target_branch = "main";
        let result = in_feature_branch(current_branch, target_branch);
        assert!(result.is_ok());
    }

    #[test]
    fn test_feature_branch_is_main_master_or_develop_should_err() {
        let test_cases = [
            ("main", "upstream-branch"),
            ("master", "upstream-branch"),
            ("develop", "upstream-branch"),
        ];

        for (current_branch, upstream_branch) in test_cases {
            let result = in_feature_branch(current_branch, upstream_branch);
            assert!(result.is_err());
        }
    }

    fn get_cmds_mock(cmd: Arc<CmdMock>) -> Vec<Cmd<CmdInfo>> {
        let cmd_status = cmd.clone();
        let git_status_cmd =
            move || -> Result<CmdInfo> { Ok(CmdInfo::StatusModified(cmd_status.status_modified)) };
        let title_cmd = cmd.clone();
        let git_title_cmd = move || -> Result<CmdInfo> {
            Ok(CmdInfo::CommitSummary(
                title_cmd.last_commit_summary.clone(),
            ))
        };
        let message_cmd = cmd.clone();
        let git_message_cmd = move || -> Result<CmdInfo> {
            Ok(CmdInfo::CommitMessage(
                message_cmd.last_commit_message.clone(),
            ))
        };
        let branch_cmd = cmd.clone();
        let git_current_branch =
            move || -> Result<CmdInfo> { Ok(CmdInfo::Branch(branch_cmd.current_branch.clone())) };
        let project_cmd = cmd.clone();
        let remote_project_cmd =
            move || -> Result<CmdInfo> { Ok(CmdInfo::Project(project_cmd.project.clone())) };
        let members_cmd = cmd.clone();
        let remote_members_cmd =
            move || -> Result<CmdInfo> { Ok(CmdInfo::Members(members_cmd.members.clone())) };
        let mut cmds: Vec<Cmd<CmdInfo>> = vec![
            Box::new(remote_project_cmd),
            Box::new(remote_members_cmd),
            Box::new(git_status_cmd),
            Box::new(git_title_cmd),
            Box::new(git_message_cmd),
            Box::new(git_current_branch),
        ];
        if cmd.error {
            let error_cmd =
                move || -> Result<CmdInfo> { Err(error::gen("Failure retrieving data")) };
            cmds.push(Box::new(error_cmd));
        }
        cmds
    }

    #[derive(Clone, Builder)]
    struct CmdMock {
        #[builder(default = "false")]
        status_modified: bool,
        last_commit_summary: String,
        current_branch: String,
        last_commit_message: String,
        members: Vec<Member>,
        project: Project,
        #[builder(default = "false")]
        error: bool,
    }

    #[test]
    fn test_get_repo_project_info() {
        let cmd_mock = CmdMockBuilder::default()
            .status_modified(true)
            .current_branch("current-branch".to_string())
            .last_commit_summary("title".to_string())
            .last_commit_message("last-commit-message".to_string())
            .members(Vec::new())
            .project(Project::default())
            .build()
            .unwrap();
        let cmds = get_cmds_mock(Arc::new(cmd_mock));
        let result = get_repo_project_info(cmds);
        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result.repo.title(), "title");
        assert_eq!(result.repo.current_branch(), "current-branch");
        assert_eq!(result.repo.last_commit_message(), "last-commit-message");
        assert_eq!(result.members.len(), 0);
    }

    #[test]
    fn test_get_repo_project_info_cmds_error() {
        let cmd_mock = CmdMockBuilder::default()
            .status_modified(true)
            .current_branch("current-branch".to_string())
            .last_commit_summary("title".to_string())
            .last_commit_message("last-commit-message".to_string())
            .members(Vec::new())
            .project(Project::default())
            .error(true)
            .build()
            .unwrap();
        let cmds = get_cmds_mock(Arc::new(cmd_mock));
        let result = get_repo_project_info(cmds);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_description_signature() {
        let description_signature_table = [
            ("", "", ""),
            ("", "signature", "signature"),
            ("description", "", "description"),
            ("description", "signature", "description\n\nsignature"),
        ];
        for (description, signature, expected) in description_signature_table {
            let result = build_description(description, signature);
            assert_eq!(result, expected);
        }
    }

    #[test]
    fn test_list_merge_requests() {
        let remote = Arc::new(
            MergeRequestRemoteMock::builder()
                .merge_requests(vec![MergeRequestResponse::builder()
                    .id(1)
                    .title("New feature".to_string())
                    .web_url("https://gitlab.com/owner/repo/-/merge_requests/1".to_string())
                    .author("author".to_string())
                    .updated_at("2021-01-01".to_string())
                    .build()
                    .unwrap()])
                .build()
                .unwrap(),
        );
        let mut buf = Vec::new();
        let body_args = MergeRequestListBodyArgs::builder()
            .list_args(None)
            .state(MergeRequestState::Opened)
            .assignee(None)
            .build()
            .unwrap();
        let cli_args = MergeRequestListCliArgs::new(
            MergeRequestState::Opened,
            ListRemoteCliArgs::builder().build().unwrap(),
        );
        list(remote, body_args, cli_args, &mut buf).unwrap();
        assert_eq!(
            "ID|Title|Source Branch|Author|URL|Updated at\n\
             1|New feature||author|https://gitlab.com/owner/repo/-/merge_requests/1|2021-01-01\n",
            String::from_utf8(buf).unwrap(),
        )
    }

    #[test]
    fn test_if_no_merge_requests_are_available_list_should_return_no_merge_requests_found() {
        let remote = Arc::new(MergeRequestRemoteMock::builder().build().unwrap());
        let mut buf = Vec::new();
        let body_args = MergeRequestListBodyArgs::builder()
            .list_args(None)
            .state(MergeRequestState::Opened)
            .assignee(None)
            .build()
            .unwrap();
        let cli_args = MergeRequestListCliArgs::new(
            MergeRequestState::Opened,
            ListRemoteCliArgs::builder().build().unwrap(),
        );
        list(remote, body_args, cli_args, &mut buf).unwrap();
        assert_eq!("No resources found.\n", String::from_utf8(buf).unwrap(),)
    }

    #[test]
    fn test_list_merge_requests_empty_with_flush_option_no_warn_message() {
        let remote = Arc::new(MergeRequestRemoteMock::builder().build().unwrap());
        let mut buf = Vec::new();
        let body_args = MergeRequestListBodyArgs::builder()
            .list_args(None)
            .state(MergeRequestState::Opened)
            .assignee(None)
            .build()
            .unwrap();
        let cli_args = MergeRequestListCliArgs::new(
            MergeRequestState::Opened,
            ListRemoteCliArgs::builder().flush(true).build().unwrap(),
        );
        list(remote, body_args, cli_args, &mut buf).unwrap();
        assert_eq!("", String::from_utf8(buf).unwrap());
    }

    #[test]
    fn test_list_merge_requests_no_headers() {
        let remote = Arc::new(
            MergeRequestRemoteMock::builder()
                .merge_requests(vec![MergeRequestResponse::builder()
                    .id(1)
                    .title("New feature".to_string())
                    .web_url("https://gitlab.com/owner/repo/-/merge_requests/1".to_string())
                    .author("author".to_string())
                    .updated_at("2021-01-01".to_string())
                    .build()
                    .unwrap()])
                .build()
                .unwrap(),
        );
        let mut buf = Vec::new();
        let body_args = MergeRequestListBodyArgs::builder()
            .list_args(None)
            .state(MergeRequestState::Opened)
            .assignee(None)
            .build()
            .unwrap();
        let cli_args = MergeRequestListCliArgs::new(
            MergeRequestState::Opened,
            ListRemoteCliArgs::builder()
                .get_args(
                    GetRemoteCliArgs::builder()
                        .no_headers(true)
                        .build()
                        .unwrap(),
                )
                .build()
                .unwrap(),
        );
        list(remote, body_args, cli_args, &mut buf).unwrap();
        assert_eq!(
            "1|New feature||author|https://gitlab.com/owner/repo/-/merge_requests/1|2021-01-01\n",
            String::from_utf8(buf).unwrap(),
        )
    }

    #[derive(Clone, Builder)]
    struct MergeRequestRemoteMock {
        #[builder(default = "Vec::new()")]
        merge_requests: Vec<MergeRequestResponse>,
    }

    impl MergeRequestRemoteMock {
        pub fn builder() -> MergeRequestRemoteMockBuilder {
            MergeRequestRemoteMockBuilder::default()
        }
    }

    impl MergeRequest for MergeRequestRemoteMock {
        fn open(&self, _args: MergeRequestBodyArgs) -> Result<MergeRequestResponse> {
            Ok(MergeRequestResponse::builder().build().unwrap())
        }
        fn list(&self, _args: MergeRequestListBodyArgs) -> Result<Vec<MergeRequestResponse>> {
            Ok(self.merge_requests.clone())
        }
        fn merge(&self, _id: i64) -> Result<MergeRequestResponse> {
            Ok(MergeRequestResponse::builder().build().unwrap())
        }
        fn get(&self, _id: i64) -> Result<MergeRequestResponse> {
            Ok(self.merge_requests[0].clone())
        }
        fn close(&self, _id: i64) -> Result<MergeRequestResponse> {
            Ok(MergeRequestResponse::builder().build().unwrap())
        }
        fn num_pages(&self, _args: MergeRequestListBodyArgs) -> Result<Option<u32>> {
            Ok(None)
        }
        fn approve(&self, _id: i64) -> Result<MergeRequestResponse> {
            Ok(self.merge_requests[0].clone())
        }

        fn num_resources(
            &self,
            _args: MergeRequestListBodyArgs,
        ) -> Result<Option<crate::api_traits::NumberDeltaErr>> {
            todo!()
        }
    }

    #[derive(Default)]
    struct MockRemoteProject {
        comment_called: Mutex<bool>,
        comment_argument: Mutex<String>,
        list_comments: Vec<Comment>,
    }

    impl MockRemoteProject {
        fn new(comments: Vec<Comment>) -> MockRemoteProject {
            MockRemoteProject {
                comment_called: Mutex::new(false),
                comment_argument: Mutex::new("".to_string()),
                list_comments: comments,
            }
        }
    }

    impl RemoteProject for MockRemoteProject {
        fn get_project_data(&self, _id: Option<i64>, _path: Option<&str>) -> Result<CmdInfo> {
            let project = Project::new(1, "main");
            Ok(CmdInfo::Project(project))
        }

        fn get_project_members(&self) -> Result<CmdInfo> {
            let members = vec![
                Member::builder()
                    .id(1)
                    .username("user1".to_string())
                    .name("User 1".to_string())
                    .build()
                    .unwrap(),
                Member::builder()
                    .id(2)
                    .username("user2".to_string())
                    .name("User 2".to_string())
                    .build()
                    .unwrap(),
            ];
            Ok(CmdInfo::Members(members))
        }

        fn get_url(&self, _option: BrowseOptions) -> String {
            todo!()
        }

        fn list(&self, _args: ProjectListBodyArgs) -> Result<Vec<Project>> {
            todo!()
        }

        fn num_pages(&self, _args: ProjectListBodyArgs) -> Result<Option<u32>> {
            todo!()
        }

        fn num_resources(
            &self,
            _args: ProjectListBodyArgs,
        ) -> Result<Option<crate::api_traits::NumberDeltaErr>> {
            todo!()
        }
    }

    impl CommentMergeRequest for MockRemoteProject {
        fn create(&self, args: CommentMergeRequestBodyArgs) -> Result<()> {
            let mut called = self.comment_called.lock().unwrap();
            *called = true;
            let mut argument = self.comment_argument.lock().unwrap();
            *argument = args.comment;
            Ok(())
        }

        fn list(&self, _args: CommentMergeRequestListBodyArgs) -> Result<Vec<Comment>> {
            Ok(self.list_comments.clone())
        }

        fn num_pages(&self, _args: CommentMergeRequestListBodyArgs) -> Result<Option<u32>> {
            todo!()
        }

        fn num_resources(
            &self,
            _args: CommentMergeRequestListBodyArgs,
        ) -> Result<Option<crate::api_traits::NumberDeltaErr>> {
            todo!()
        }
    }

    struct MockShellRunner {
        responses: Mutex<Vec<Response>>,
    }

    impl MockShellRunner {
        pub fn new(response: Vec<Response>) -> MockShellRunner {
            MockShellRunner {
                responses: Mutex::new(response),
            }
        }
    }

    impl TaskRunner for MockShellRunner {
        type Response = Response;

        fn run<T>(&self, _cmd: T) -> Result<Self::Response>
        where
            T: IntoIterator,
            T::Item: AsRef<std::ffi::OsStr>,
        {
            let response = self.responses.lock().unwrap().pop().unwrap();
            Ok(Response::builder().body(response.body).build().unwrap())
        }
    }

    fn gen_cmd_responses() -> Vec<Response> {
        let responses = vec![
            Response::builder()
                .body("fetch cmd".to_string())
                .build()
                .unwrap(),
            Response::builder()
                .body("last commit message cmd".to_string())
                .build()
                .unwrap(),
            Response::builder()
                .body("current branch cmd".to_string())
                .build()
                .unwrap(),
            Response::builder()
                .body("title git cmd".to_string())
                .build()
                .unwrap(),
            Response::builder()
                .body("status cmd".to_string())
                .build()
                .unwrap(),
        ];
        responses
    }

    #[test]
    fn test_cmds_gather_title_from_cli_arg() {
        let remote = Arc::new(MockRemoteProject::default());
        let cli_args = MergeRequestCliArgs::builder()
            .title(Some("title cli".to_string()))
            .title_from_commit(None)
            .description(None)
            .description_from_file(None)
            .target_branch(Some("target-branch".to_string()))
            .auto(false)
            .cache_args(CacheCliArgs::default())
            .open_browser(false)
            .accept_summary(false)
            .commit(Some("commit".to_string()))
            .draft(false)
            .force(false)
            .amend(false)
            .build()
            .unwrap();

        let responses = gen_cmd_responses();

        let task_runner = Arc::new(MockShellRunner::new(responses));
        let cmds = cmds(remote, &cli_args, task_runner, None::<Cursor<&str>>);
        assert_eq!(cmds.len(), 6);
        let cmds = cmds
            .into_iter()
            .map(|cmd| cmd())
            .collect::<Result<Vec<CmdInfo>>>()
            .unwrap();
        let title_result = cmds[2].clone();
        let title = match title_result {
            CmdInfo::CommitSummary(title) => title,
            _ => "".to_string(),
        };
        assert_eq!("title cli", title);
    }

    #[test]
    fn test_cmds_gather_title_from_git_commit_summary() {
        let remote = Arc::new(MockRemoteProject::default());
        let cli_args = MergeRequestCliArgs::builder()
            .title(None)
            .title_from_commit(None)
            .description(None)
            .description_from_file(None)
            .target_branch(Some("target-branch".to_string()))
            .auto(false)
            .cache_args(CacheCliArgs::default())
            .open_browser(false)
            .accept_summary(false)
            .commit(None)
            .draft(false)
            .force(false)
            .amend(false)
            .build()
            .unwrap();

        let responses = gen_cmd_responses();

        let task_runner = Arc::new(MockShellRunner::new(responses));

        let cmds = cmds(remote, &cli_args, task_runner, None::<Cursor<&str>>);
        let results = cmds
            .into_iter()
            .map(|cmd| cmd())
            .collect::<Result<Vec<CmdInfo>>>()
            .unwrap();
        let title_result = results[2].clone();
        let title = match title_result {
            CmdInfo::CommitSummary(title) => title,
            _ => "".to_string(),
        };
        assert_eq!("title git cmd", title);
    }

    #[test]
    fn test_read_description_from_file() {
        let remote = Arc::new(MockRemoteProject::default());
        let cli_args = MergeRequestCliArgs::builder()
            .title(None)
            .title_from_commit(None)
            .description(None)
            .description_from_file(Some("description_file.txt".to_string()))
            .target_branch(Some("target-branch".to_string()))
            .auto(false)
            .cache_args(CacheCliArgs::default())
            .open_browser(false)
            .accept_summary(false)
            .commit(None)
            .draft(false)
            .force(false)
            .amend(false)
            .build()
            .unwrap();

        let responses = gen_cmd_responses();

        let task_runner = Arc::new(MockShellRunner::new(responses));

        let description_contents = "This merge requests adds a new feature\n";
        let reader = Cursor::new(description_contents);
        let cmds = cmds(remote, &cli_args, task_runner, Some(reader));
        let results = cmds
            .into_iter()
            .map(|cmd| cmd())
            .collect::<Result<Vec<CmdInfo>>>()
            .unwrap();
        let description_result = results[4].clone();
        let description = match description_result {
            CmdInfo::CommitMessage(description) => description,
            _ => "".to_string(),
        };
        assert_eq!(description_contents, description);
    }

    #[test]
    fn test_create_comment_on_a_merge_request_with_cli_comment_ok() {
        let remote = Arc::new(MockRemoteProject::default());
        let cli_args = CommentMergeRequestCliArgs::builder()
            .id(1)
            .comment(Some("All features complete, ship it".to_string()))
            .comment_from_file(None)
            .build()
            .unwrap();
        let reader = Cursor::new("comment");
        assert!(create_comment(remote.clone(), cli_args, Some(reader)).is_ok());
        assert!(remote.comment_called.lock().unwrap().clone());
        assert_eq!(
            "All features complete, ship it",
            remote.comment_argument.lock().unwrap().clone(),
        );
    }

    #[test]
    fn test_create_comment_on_a_merge_request_with_comment_from_file_ok() {
        let remote = Arc::new(MockRemoteProject::default());
        let cli_args = CommentMergeRequestCliArgs::builder()
            .id(1)
            .comment(None)
            .comment_from_file(Some("comment_file.txt".to_string()))
            .build()
            .unwrap();
        let reader = Cursor::new("Just a long, long comment from a file");
        assert!(create_comment(remote.clone(), cli_args, Some(reader)).is_ok());
        assert!(remote.comment_called.lock().unwrap().clone());
        assert_eq!(
            "Just a long, long comment from a file",
            remote.comment_argument.lock().unwrap().clone(),
        );
    }

    struct ErrorReader {}

    impl Read for ErrorReader {
        fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Error reading from reader",
            ))
        }
    }

    impl BufRead for ErrorReader {
        fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Error reading from reader",
            ))
        }
        fn consume(&mut self, _amt: usize) {}
    }

    #[test]
    fn test_create_comment_on_a_merge_request_fail_to_read_comment_from_file() {
        let remote = Arc::new(MockRemoteProject::default());
        let cli_args = CommentMergeRequestCliArgs::builder()
            .id(1)
            .comment(None)
            .comment_from_file(Some("comment_file.txt".to_string()))
            .build()
            .unwrap();
        let reader = ErrorReader {};
        assert!(create_comment(remote.clone(), cli_args, Some(reader)).is_err());
    }

    #[test]
    fn test_get_merge_request_details() {
        let cli_args = MergeRequestGetCliArgs::builder()
            .id(1)
            .get_args(
                GetRemoteCliArgs::builder()
                    .display_optional(true)
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        let response = MergeRequestResponse::builder()
            .id(1)
            .title("New feature".to_string())
            .web_url("https://gitlab.com/owner/repo/-/merge_requests/1".to_string())
            .description("Implement get merge request".to_string())
            .merged_at("2024-03-03T00:00:00Z".to_string())
            .pipeline_id(Some(1))
            .pipeline_url(Some(
                "https://gitlab.com/owner/repo/-/pipelines/1".to_string(),
            ))
            .build()
            .unwrap();
        let remote = Arc::new(
            MergeRequestRemoteMock::builder()
                .merge_requests(vec![response])
                .build()
                .unwrap(),
        );
        let mut writer = Vec::new();
        get_merge_request_details(remote, cli_args, &mut writer).unwrap();
        assert_eq!(
            "ID|Title|Source Branch|SHA|Description|Author|URL|Updated at|Merged at|Pipeline ID|Pipeline URL\n\
             1|New feature|||Implement get merge request||https://gitlab.com/owner/repo/-/merge_requests/1||2024-03-03T00:00:00Z|1|https://gitlab.com/owner/repo/-/pipelines/1\n",
            String::from_utf8(writer).unwrap(),
        )
    }

    #[test]
    fn test_approve_merge_request_ok() {
        let approve_response = MergeRequestResponse::builder()
            .id(1)
            .web_url("https://gitlab.com/owner/repo/-/merge_requests/1".to_string())
            .build()
            .unwrap();
        let remote = Arc::new(
            MergeRequestRemoteMock::builder()
                .merge_requests(vec![approve_response])
                .build()
                .unwrap(),
        );
        let mut writer = Vec::new();
        approve(remote, 1, &mut writer).unwrap();
        assert_eq!(
            "Merge request approved: https://gitlab.com/owner/repo/-/merge_requests/1\n",
            String::from_utf8(writer).unwrap(),
        );
    }

    #[test]
    fn test_cmds_fetch_cli_arg() {
        let remote = Arc::new(MockRemoteProject::default());
        let cli_args = MergeRequestCliArgs::builder()
            .title(Some("title cli".to_string()))
            .title_from_commit(None)
            .description(None)
            .description_from_file(None)
            .target_branch(Some("target-branch".to_string()))
            .fetch(Some("origin".to_string()))
            .auto(false)
            .cache_args(CacheCliArgs::default())
            .open_browser(false)
            .accept_summary(false)
            .commit(Some("commit".to_string()))
            .draft(false)
            .force(false)
            .amend(false)
            .build()
            .unwrap();

        let responses = gen_cmd_responses();

        let task_runner = Arc::new(MockShellRunner::new(responses));
        let cmds = cmds(remote, &cli_args, task_runner, None::<Cursor<&str>>);
        assert_eq!(cmds.len(), 7);
        let cmds = cmds
            .into_iter()
            .map(|cmd| cmd())
            .collect::<Result<Vec<CmdInfo>>>()
            .unwrap();
        let fetch_result = cmds[6].clone();
        match fetch_result {
            CmdInfo::Ignore => {}
            _ => panic!("Expected ignore cmdinfo variant on fetch"),
        };
    }

    #[test]
    fn test_list_merge_request_comments() {
        let comments = vec![
            Comment::builder()
                .id(1)
                .body("Great work!".to_string())
                .author("user1".to_string())
                .created_at("2021-01-01".to_string())
                .build()
                .unwrap(),
            Comment::builder()
                .id(2)
                .body("Keep it up!".to_string())
                .author("user2".to_string())
                .created_at("2021-01-02".to_string())
                .build()
                .unwrap(),
        ];
        let remote = Arc::new(MockRemoteProject::new(comments));
        let body_args = CommentMergeRequestListBodyArgs::builder()
            .id(1)
            .list_args(None)
            .build()
            .unwrap();
        let cli_args = CommentMergeRequestListCliArgs::builder()
            .id(1)
            .list_args(ListRemoteCliArgs::builder().build().unwrap())
            .build()
            .unwrap();
        let mut buf = Vec::new();
        list_comments(remote, body_args, cli_args, &mut buf).unwrap();
        assert_eq!(
            "ID|Body|Author|Created at\n\
             1|Great work!|user1|2021-01-01\n\
             2|Keep it up!|user2|2021-01-02\n",
            String::from_utf8(buf).unwrap(),
        );
    }
}
