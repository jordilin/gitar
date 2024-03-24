use crate::api_traits::{CommentMergeRequest, MergeRequest, RemoteProject};
use crate::cli::merge_request::MergeRequestOptions;
use crate::config::{Config, ConfigProperties};
use crate::error::{AddContext, GRError};
use crate::git::Repo;
use crate::io::{CmdInfo, Response, TaskRunner};
use crate::remote::{
    GetRemoteCliArgs, ListRemoteCliArgs, Member, MergeRequestBodyArgs, MergeRequestListBodyArgs,
    MergeRequestState, Project,
};
use crate::shell::Shell;
use crate::{dialog, display, exec, git, remote, Cmd, Result};
use std::{
    fs::File,
    io::{BufRead, BufReader, Cursor, Write},
    sync::Arc,
};

use super::common::process_num_pages;

#[derive(Builder, Clone)]
pub struct MergeRequestCliArgs {
    pub title: Option<String>,
    pub title_from_commit: Option<String>,
    pub description: Option<String>,
    pub description_from_file: Option<String>,
    pub target_branch: Option<String>,
    pub auto: bool,
    pub refresh_cache: bool,
    pub open_browser: bool,
    pub accept_summary: bool,
    pub commit: Option<String>,
    pub draft: bool,
}

impl MergeRequestCliArgs {
    pub fn builder() -> MergeRequestCliArgsBuilder {
        MergeRequestCliArgsBuilder::default()
    }
}

pub struct MergeRequestListCliArgs {
    pub state: MergeRequestState,
    pub list_args: ListRemoteCliArgs,
}

impl MergeRequestListCliArgs {
    pub fn new(state: MergeRequestState, args: ListRemoteCliArgs) -> MergeRequestListCliArgs {
        MergeRequestListCliArgs {
            state,
            list_args: args,
        }
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
pub struct CommentMergeRequestBodyArgs {
    pub id: i64,
    pub comment: String,
}

impl CommentMergeRequestBodyArgs {
    pub fn builder() -> CommentMergeRequestBodyArgsBuilder {
        CommentMergeRequestBodyArgsBuilder::default()
    }
}

pub fn execute(
    options: MergeRequestOptions,
    config: Arc<Config>,
    domain: String,
    path: String,
) -> Result<()> {
    match options {
        MergeRequestOptions::Create(cli_args) => {
            let mr_remote = remote::get_mr(
                domain.clone(),
                path.clone(),
                config.clone(),
                cli_args.refresh_cache,
            )?;
            let project_remote =
                remote::get_project(domain, path, config.clone(), cli_args.refresh_cache)?;
            if let Some(commit_message) = &cli_args.commit {
                git::add(&Shell)?;
                git::commit(&Shell, commit_message)?;
            }
            let cmds = if let Some(description_file) = &cli_args.description_from_file {
                let reader = get_reader_file_cli(description_file)?;
                cmds(project_remote, &cli_args, Arc::new(Shell), Some(reader))
            } else {
                cmds(
                    project_remote,
                    &cli_args,
                    Arc::new(Shell),
                    None::<Cursor<&str>>,
                )
            };
            let mr_body = get_repo_project_info(cmds)?;
            open(mr_remote, config, mr_body, &cli_args)
        }
        MergeRequestOptions::List(cli_args) => {
            list_merge_requests(domain, path, config, cli_args, None)
        }
        MergeRequestOptions::Merge { id } => {
            let remote = remote::get_mr(domain, path, config, false)?;
            merge(remote, id)
        }
        MergeRequestOptions::Checkout { id } => {
            let remote = remote::get_mr(domain, path, config, false)?;
            checkout(remote, id)
        }
        MergeRequestOptions::Close { id } => {
            let remote = remote::get_mr(domain, path, config, false)?;
            close(remote, id)
        }
        MergeRequestOptions::Comment(cli_args) => {
            let remote = remote::get_comment_mr(domain, path, config, false)?;
            if let Some(comment_file) = &cli_args.comment_from_file {
                let reader = get_reader_file_cli(comment_file)?;
                create_comment(remote, cli_args, Some(reader))
            } else {
                create_comment(remote, cli_args, None::<Cursor<&str>>)
            }
        }
        MergeRequestOptions::Get(cli_args) => {
            let remote = remote::get_mr(domain, path, config, cli_args.get_args.refresh_cache)?;
            get_merge_request_details(remote, cli_args, std::io::stdout())
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

pub fn list_merge_requests(
    domain: String,
    path: String,
    config: Arc<Config>,
    cli_args: MergeRequestListCliArgs,
    assignee_id: Option<i64>,
) -> Result<()> {
    let remote = remote::get_mr(
        domain,
        path,
        config,
        cli_args.list_args.get_args.refresh_cache,
    )?;
    let from_to_args = remote::validate_from_to_page(&cli_args.list_args)?;
    let body_args = MergeRequestListBodyArgs::builder()
        .list_args(from_to_args)
        .state(cli_args.state)
        .assignee_id(assignee_id)
        .build()?;
    if cli_args.list_args.num_pages {
        return process_num_pages(remote.num_pages(body_args), std::io::stdout());
    }
    list(remote, body_args, cli_args, std::io::stdout())
}

fn user_prompt_confirmation(
    mr_body: &MergeRequestBody,
    config: Arc<impl ConfigProperties>,
    description: String,
    target_branch: &String,
    cli_args: &MergeRequestCliArgs,
) -> Result<MergeRequestBodyArgs> {
    let mut title = mr_body.repo.title().to_string();
    if cli_args.draft {
        title = format!("DRAFT: {}", title);
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
        .build()?)
}

/// Open a merge request.
fn open(
    remote: Arc<dyn MergeRequest>,
    config: Arc<impl ConfigProperties>,
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

    git::rebase(&Shell, "origin", &target_branch)?;

    let outgoing_commits = git::outgoing_commits(&Shell, "origin", &target_branch)?;

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
        git::push(&Shell, "origin", &mr_body.repo)?;
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
    let remote_project_cmd = move || -> Result<CmdInfo> { remote_cl.get_project_data(None) };
    let remote_members_cmd = move || -> Result<CmdInfo> { remote.get_project_members() };
    let status_runner = task_runner.clone();
    let git_status_cmd = || -> Result<CmdInfo> { git::status(status_runner) };
    let fetch_runner = task_runner.clone();
    let git_fetch_cmd = || -> Result<CmdInfo> { git::fetch(fetch_runner) };
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
    let cmds: Vec<Cmd<CmdInfo>> = vec![
        Box::new(remote_project_cmd),
        Box::new(remote_members_cmd),
        Box::new(git_status_cmd),
        Box::new(git_fetch_cmd),
        Box::new(git_title_cmd),
        Box::new(git_current_branch),
        Box::new(git_last_commit_message),
    ];
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
    let merge_requests = remote.list(body_args)?;
    if merge_requests.is_empty() {
        writer.write_all(b"No merge requests found.\n")?;
        return Ok(());
    }
    display::print(&mut writer, merge_requests, cli_args.list_args.get_args)?;
    Ok(())
}

fn merge(remote: Arc<dyn MergeRequest>, merge_request_id: i64) -> Result<()> {
    let merge_request = remote.merge(merge_request_id)?;
    println!("Merge request merged: {}", merge_request.web_url);
    Ok(())
}

fn checkout(remote: Arc<dyn MergeRequest>, id: i64) -> Result<()> {
    let merge_request = remote.get(id)?;
    git::fetch(Arc::new(Shell))?;
    git::checkout(&Shell, &merge_request.mr.source_branch)
}

fn close(remote: Arc<dyn MergeRequest>, id: i64) -> Result<()> {
    let merge_request = remote.close(id)?;
    println!("Merge request closed: {}", merge_request.web_url);
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

#[cfg(test)]
mod tests {
    use std::{
        io::{Cursor, Read},
        sync::Mutex,
    };

    use crate::{
        api_traits::CommentMergeRequest, cli::browse::BrowseOptions, error,
        remote::MergeRequestResponse,
    };

    use self::remote::MergeRequestMetadata;

    use super::*;

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
            .assignee_id(None)
            .build()
            .unwrap();
        let cli_args = MergeRequestListCliArgs::new(
            MergeRequestState::Opened,
            ListRemoteCliArgs::builder().build().unwrap(),
        );
        list(remote, body_args, cli_args, &mut buf).unwrap();
        assert_eq!(
            "ID|Title|Author|URL|Updated at\n\
             1|New feature|author|https://gitlab.com/owner/repo/-/merge_requests/1|2021-01-01\n",
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
            .assignee_id(None)
            .build()
            .unwrap();
        let cli_args = MergeRequestListCliArgs::new(
            MergeRequestState::Opened,
            ListRemoteCliArgs::builder().build().unwrap(),
        );
        list(remote, body_args, cli_args, &mut buf).unwrap();
        assert_eq!(
            "No merge requests found.\n",
            String::from_utf8(buf).unwrap(),
        )
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
            .assignee_id(None)
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
            "1|New feature|author|https://gitlab.com/owner/repo/-/merge_requests/1|2021-01-01\n",
            String::from_utf8(buf).unwrap(),
        )
    }

    #[derive(Clone, Builder)]
    struct MergeRequestRemoteMock {
        #[builder(default = "Vec::new()")]
        merge_requests: Vec<MergeRequestResponse>,
        #[builder(default)]
        merge_request_metatada: MergeRequestMetadata,
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
        fn get(&self, _id: i64) -> Result<MergeRequestMetadata> {
            Ok(self.merge_request_metatada.clone())
        }
        fn close(&self, _id: i64) -> Result<MergeRequestResponse> {
            Ok(MergeRequestResponse::builder().build().unwrap())
        }
        fn num_pages(&self, _args: MergeRequestListBodyArgs) -> Result<Option<u32>> {
            Ok(None)
        }
    }

    #[derive(Default)]
    struct MockRemoteProject {
        comment_called: Mutex<bool>,
        comment_argument: Mutex<String>,
    }
    impl RemoteProject for MockRemoteProject {
        fn get_project_data(&self, _id: Option<i64>) -> Result<CmdInfo> {
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
    }

    impl CommentMergeRequest for MockRemoteProject {
        fn create(&self, args: CommentMergeRequestBodyArgs) -> Result<()> {
            let mut called = self.comment_called.lock().unwrap();
            *called = true;
            let mut argument = self.comment_argument.lock().unwrap();
            *argument = args.comment;
            Ok(())
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
                .body("fetch cmd".to_string())
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
            .refresh_cache(false)
            .open_browser(false)
            .accept_summary(false)
            .commit(Some("commit".to_string()))
            .draft(false)
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
        let title_result = cmds[4].clone();
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
            .refresh_cache(false)
            .open_browser(false)
            .accept_summary(false)
            .commit(None)
            .draft(false)
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
        let title_result = results[4].clone();
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
            .refresh_cache(false)
            .open_browser(false)
            .accept_summary(false)
            .commit(None)
            .draft(false)
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
        let description_result = results[6].clone();
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
            .get_args(GetRemoteCliArgs::builder().build().unwrap())
            .build()
            .unwrap();
        let response = MergeRequestMetadata::builder()
            .mr(MergeRequestResponse::builder()
                .id(1)
                .title("New feature".to_string())
                .web_url("https://gitlab.com/owner/repo/-/merge_requests/1".to_string())
                .build()
                .unwrap())
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
                .merge_request_metatada(response)
                .build()
                .unwrap(),
        );
        let mut writer = Vec::new();
        get_merge_request_details(remote, cli_args, &mut writer).unwrap();
        assert_eq!(
            "ID|Title|Description|URL|Merged at|Pipeline ID|Pipeline URL\n\
             1|New feature|Implement get merge request|https://gitlab.com/owner/repo/-/merge_requests/1|2024-03-03T00:00:00Z|1|https://gitlab.com/owner/repo/-/pipelines/1\n",
            String::from_utf8(writer).unwrap(),
        )
    }
}
