use std::sync::Arc;

use crate::api_traits::Remote;

use crate::config::ConfigProperties;
use crate::error::GRError;
use crate::exec;
use crate::remote::Member;
use crate::remote::MergeRequestArgs;
use crate::remote::MergeRequestState;
use crate::remote::Project;
use crate::shell::Shell;

use crate::dialog;

use crate::git;

use crate::io::CmdInfo;
use crate::Cmd;
use crate::Result;

/// Open a merge request.
pub fn open(
    remote: Arc<dyn Remote>,
    config: Arc<impl ConfigProperties>,
    title: Option<String>,
    description: Option<String>,
    target_branch: Option<String>,
    noprompt: bool,
) -> Result<()> {
    // data gathering stage. Gather local repo and remote project data.

    let data = cmds(remote.clone());
    let mut project = Project::default();
    let mut members = Vec::new();
    let mut repo = git::Repo::default();
    let cmd_results = exec::parallel_stream(data);
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
            Ok(CmdInfo::LastCommitSummary(title)) => repo.with_title(&title),
            Ok(CmdInfo::LastCommitMessage(message)) => repo.with_last_commit_message(&message),
            // bail on first error found
            Err(e) => return Err(e),
            _ => {}
        }
    }
    let source_branch = &repo.current_branch();
    let target_branch = &target_branch.unwrap_or(project.default_branch().to_string());
    let title = title.unwrap_or(repo.title().to_string());
    let description = description.unwrap_or(repo.last_commit_message().to_string());
    let preferred_assignee_username = config.preferred_assignee_username().to_string();

    // append description signature from the configuration
    let description =
        if description.is_empty() && config.merge_request_description_signature().is_empty() {
            description
        } else if description.is_empty() {
            config.merge_request_description_signature().to_string()
        } else if config.merge_request_description_signature().is_empty() {
            description
        } else {
            format!(
                "{}\n\n{}",
                description,
                config.merge_request_description_signature()
            )
        };

    // make sure we are in a feature branch or bail
    in_feature_branch(source_branch, target_branch)?;

    // confirm title, description and assignee
    let user_input = if noprompt {
        let preferred_assignee_members = members
            .iter()
            .filter(|member| member.username == preferred_assignee_username)
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
        dialog::prompt_user_merge_request_info(&title, &description, &members, config)?
    };

    git::rebase(&Shell, "origin", target_branch)?;

    let outgoing_commits = git::outgoing_commits(&Shell, "origin", target_branch)?;

    if outgoing_commits.is_empty() {
        return Err(GRError::PreconditionNotMet(
            "No outgoing commits found. Please commit your changes.".to_string(),
        )
        .into());
    }

    let args = MergeRequestArgs::new()
        .with_title(&user_input.title)
        .with_description(&user_input.description)
        .with_source_branch(source_branch)
        .with_target_branch(target_branch)
        .with_assignee_id(&user_input.user_id.to_string())
        .with_username(&user_input.username);

    // show summary of merge request and confirm
    if let Ok(()) = dialog::show_summary_merge_request(&outgoing_commits, &args) {
        println!("\nTaking off... 🚀\n");
        git::push(&Shell, "origin", &repo)?;
        let merge_request_response = remote.open(args)?;
        println!("Merge request opened: {}", merge_request_response.web_url);
    }
    Ok(())
}

/// Required commands to build a Project and a Repository
///
/// The order of the commands being declared is not important as they will be
/// executed in parallel.
fn cmds(remote: Arc<dyn Remote>) -> Vec<Cmd<CmdInfo>> {
    let remote_cl = remote.clone();
    let remote_project_cmd = move || -> Result<CmdInfo> { remote_cl.get_project_data() };
    let remote_members_cmd = move || -> Result<CmdInfo> { remote.get_project_members() };
    let git_status_cmd = || -> Result<CmdInfo> { git::status(&Shell) };
    let git_fetch_cmd = || -> Result<CmdInfo> { git::fetch(&Shell) };
    let git_title_cmd = || -> Result<CmdInfo> { git::last_commit(&Shell) };
    let git_current_branch = || -> Result<CmdInfo> { git::current_branch(&Shell) };
    let git_last_commit_message = || -> Result<CmdInfo> { git::last_commit_message(&Shell) };
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

/// This makes sure we don't push to branches considered to be upstream in most cases.
pub fn in_feature_branch(current_branch: &str, upstream_branch: &str) -> Result<()> {
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

pub fn list(remote: Arc<dyn Remote>, state: MergeRequestState) -> Result<()> {
    let merge_requests = remote.list(state)?;
    if merge_requests.is_empty() {
        println!("No merge requests found.");
        return Ok(());
    }
    println!("ID | URL | Author | Updated at");
    for mr in merge_requests {
        println!(
            "{} | {} | {} | {}",
            mr.id, mr.web_url, mr.author, mr.updated_at
        );
    }
    Ok(())
}

pub fn merge(remote: Arc<dyn Remote>, merge_request_id: i64) -> Result<()> {
    let merge_request = remote.merge(merge_request_id)?;
    println!("Merge request merged: {}", merge_request.web_url);
    Ok(())
}

pub fn checkout(remote: Arc<dyn Remote>, id: i64) -> Result<()> {
    let merge_request = remote.get(id)?;
    git::fetch(&Shell)?;
    git::checkout(&Shell, &merge_request.source_branch)
}

pub fn close(remote: Arc<dyn Remote>, id: i64) -> Result<()> {
    let merge_request = remote.close(id)?;
    println!("Merge request closed: {}", merge_request.web_url);
    Ok(())
}

#[cfg(test)]
mod tests {
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
}
