use std::io::Write;
use std::sync::Arc;

use crate::api_traits::MergeRequest;
use crate::api_traits::RemoteProject;
use crate::config::ConfigProperties;
use crate::display;
use crate::error::GRError;
use crate::exec;
use crate::git::Repo;
use crate::remote::ListRemoteCliArgs;
use crate::remote::Member;
use crate::remote::MergeRequestBodyArgs;
use crate::remote::MergeRequestListBodyArgs;
use crate::remote::MergeRequestState;
use crate::remote::Project;
use crate::shell::Shell;

use crate::dialog;

use crate::git;

use crate::cli::MergeRequestOptions;
use crate::config::Config;
use crate::io::CmdInfo;
use crate::remote;
use crate::Cmd;
use crate::Result;

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
            let mr_body = get_repo_project_info(cmds(project_remote, &cli_args))?;
            open(mr_remote, config, mr_body, &cli_args)
        }
        MergeRequestOptions::List(cli_args) => {
            let remote = remote::get_mr(domain, path, config, cli_args.list_args.refresh_cache)?;
            let from_to_args = remote::validate_from_to_page(&cli_args.list_args)?;
            let body_args = MergeRequestListBodyArgs::builder()
                .list_args(from_to_args)
                .state(cli_args.state)
                .build()?;
            if cli_args.list_args.num_pages {
                return process_num_pages(remote.num_pages(body_args), std::io::stdout());
            }
            list(remote, body_args, cli_args, std::io::stdout())
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
    }
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
///
/// The order of the commands being declared is not important as they will be
/// executed in parallel.
fn cmds(
    remote: Arc<dyn RemoteProject + Send + Sync + 'static>,
    cli_args: &MergeRequestCliArgs,
) -> Vec<Cmd<CmdInfo>> {
    let remote_cl = remote.clone();
    let remote_project_cmd = move || -> Result<CmdInfo> { remote_cl.get_project_data(None) };
    let remote_members_cmd = move || -> Result<CmdInfo> { remote.get_project_members() };
    let git_status_cmd = || -> Result<CmdInfo> { git::status(&Shell) };
    let git_fetch_cmd = || -> Result<CmdInfo> { git::fetch(&Shell) };
    let title = cli_args.title.clone();
    let title = title.unwrap_or("".to_string());
    let title_from_commit = cli_args.title_from_commit.clone();
    // if we are required to gather the title from specific commit, gather also
    // its description. The description will be pulled from the same commit as
    // the title.
    let description_commit = cli_args.title_from_commit.clone();
    let git_title_cmd = move || -> Result<CmdInfo> {
        if title.is_empty() {
            git::commit_summary(&Shell, &title_from_commit)
        } else {
            Ok(CmdInfo::CommitSummary(title.clone()))
        }
    };
    let git_current_branch = || -> Result<CmdInfo> { git::current_branch(&Shell) };
    let description = cli_args.description.clone();
    let description = description.unwrap_or("".to_string());
    let git_last_commit_message = move || -> Result<CmdInfo> {
        if description.is_empty() {
            git::commit_message(&Shell, &description_commit)
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
    display::print(
        &mut writer,
        merge_requests,
        cli_args.list_args.no_headers,
        &cli_args.list_args.format,
    )?;
    Ok(())
}

fn merge(remote: Arc<dyn MergeRequest>, merge_request_id: i64) -> Result<()> {
    let merge_request = remote.merge(merge_request_id)?;
    println!("Merge request merged: {}", merge_request.web_url);
    Ok(())
}

fn checkout(remote: Arc<dyn MergeRequest>, id: i64) -> Result<()> {
    let merge_request = remote.get(id)?;
    git::fetch(&Shell)?;
    git::checkout(&Shell, &merge_request.source_branch)
}

fn close(remote: Arc<dyn MergeRequest>, id: i64) -> Result<()> {
    let merge_request = remote.close(id)?;
    println!("Merge request closed: {}", merge_request.web_url);
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{error, remote::MergeRequestResponse};

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
            MergeRequestListMock::builder()
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
            .build()
            .unwrap();
        let cli_args = MergeRequestListCliArgs::new(
            MergeRequestState::Opened,
            ListRemoteCliArgs::builder().build().unwrap(),
        );
        list(remote, body_args, cli_args, &mut buf).unwrap();
        assert_eq!(
            "ID | Title | Author | URL | Updated at\n\
             1 | New feature | author | https://gitlab.com/owner/repo/-/merge_requests/1 | 2021-01-01\n",
            String::from_utf8(buf).unwrap(),
        )
    }

    #[test]
    fn test_if_no_merge_requests_are_available_list_should_return_no_merge_requests_found() {
        let remote = Arc::new(MergeRequestListMock::builder().build().unwrap());
        let mut buf = Vec::new();
        let body_args = MergeRequestListBodyArgs::builder()
            .list_args(None)
            .state(MergeRequestState::Opened)
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
            MergeRequestListMock::builder()
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
            .build()
            .unwrap();
        let cli_args = MergeRequestListCliArgs::new(
            MergeRequestState::Opened,
            ListRemoteCliArgs::builder()
                .no_headers(true)
                .build()
                .unwrap(),
        );
        list(remote, body_args, cli_args, &mut buf).unwrap();
        assert_eq!(
            "1 | New feature | author | https://gitlab.com/owner/repo/-/merge_requests/1 | 2021-01-01\n",
            String::from_utf8(buf).unwrap(),
        )
    }

    #[derive(Clone, Builder)]
    struct MergeRequestListMock {
        #[builder(default = "Vec::new()")]
        merge_requests: Vec<MergeRequestResponse>,
    }

    impl MergeRequestListMock {
        pub fn builder() -> MergeRequestListMockBuilder {
            MergeRequestListMockBuilder::default()
        }
    }

    impl MergeRequest for MergeRequestListMock {
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
            Ok(MergeRequestResponse::builder().build().unwrap())
        }
        fn close(&self, _id: i64) -> Result<MergeRequestResponse> {
            Ok(MergeRequestResponse::builder().build().unwrap())
        }
        fn num_pages(&self, _args: MergeRequestListBodyArgs) -> Result<Option<u32>> {
            Ok(None)
        }
    }
}
