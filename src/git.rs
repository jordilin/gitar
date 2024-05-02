//! Git commands. It defines the public entrypoints to execute git commands and
//! interact with the git repositories.
//!
//! Git commands are just public functions that return a [`Result<CmdInfo>`].
//! The [`CmdInfo`] is an enum that defines the different types of information
//! returned by git operations. These operations can execute in parallel and the
//! result can be combined at the end to make a decission before opening a merge
//! request.
//!
//! All public functions take a [`Runner`] as a parameter and return a
//! [`Result<CmdInfo>`].

use std::sync::Arc;

use crate::error;
use crate::error::AddContext;
use crate::io::CmdInfo;
use crate::io::Response;
use crate::io::TaskRunner;
use crate::Result;

/// Gather the status of the local git repository.
///
/// Takes a [`Runner`] as a parameter and returns a result encapsulating a
/// [`CmdInfo::StatusModified`]. Untracked files are not being considered.
pub fn status(exec: Arc<impl TaskRunner<Response = Response>>) -> Result<CmdInfo> {
    let cmd_params = ["git", "status", "--short"];
    let response = exec.run(cmd_params)?;
    handle_git_status(&response)
}

fn handle_git_status(response: &Response) -> Result<CmdInfo> {
    let modified = response
        .body
        .split('\n')
        .filter(|s| {
            let fields = s.split(' ').collect::<Vec<&str>>();
            if fields.len() == 3 && fields[1] == "M" {
                return true;
            }
            false
        })
        .count();
    if modified > 0 {
        return Ok(CmdInfo::StatusModified(true));
    }
    Ok(CmdInfo::StatusModified(false))
}

/// Gather the current branch name in the local git repository.
pub fn current_branch(runner: Arc<impl TaskRunner<Response = Response>>) -> Result<CmdInfo> {
    // Does not work for git version at least 2.18
    // let cmd_params = ["git", "branch", "--show-current"];
    // Use rev-parse for older versions of git that don't support --show-current.
    let cmd_params = ["git", "rev-parse", "--abbrev-ref", "HEAD"];
    let response = runner.run(cmd_params).err_context(format!(
        "Failed to get current branch. Command: {}",
        cmd_params.join(" ")
    ))?;
    Ok(CmdInfo::Branch(response.body))
}

/// Fetch the last commits from the remote.
///
/// The remote is considered to be the default remote, .i.e origin.
/// Takes a [`Runner`] as a parameter and the encapsulated result is a
/// [`CmdInfo::Ignore`].
pub fn fetch(exec: Arc<impl TaskRunner>, remote_alias: String) -> Result<CmdInfo> {
    let cmd_params = ["git", "fetch", &remote_alias];
    exec.run(cmd_params).err_context(format!(
        "Failed to git fetch. Command: {}",
        cmd_params.join(" ")
    ))?;
    Ok(CmdInfo::Ignore)
}

pub fn add(exec: &impl TaskRunner) -> Result<CmdInfo> {
    let cmd_params = ["git", "add", "-u"];
    exec.run(cmd_params).err_context(format!(
        "Failed to git add changes. Command: {}",
        cmd_params.join(" ")
    ))?;
    Ok(CmdInfo::Ignore)
}

pub fn commit(exec: &impl TaskRunner, message: &str) -> Result<CmdInfo> {
    let cmd_params = ["git", "commit", "-m", message];
    exec.run(cmd_params).err_context(format!(
        "Failed to git commit changes. Command: {}",
        cmd_params.join(" ")
    ))?;
    Ok(CmdInfo::Ignore)
}

/// Get the origin remote url from the local git repository.
pub fn remote_url(exec: &impl TaskRunner<Response = Response>) -> Result<CmdInfo> {
    let cmd_params = ["git", "remote", "get-url", "--all", "origin"];
    let response = exec.run(cmd_params)?;
    handle_git_remote_url(&response)
}

fn handle_git_remote_url(response: &Response) -> Result<CmdInfo> {
    let fields = response.body.split(':').collect::<Vec<&str>>();
    match fields.len() {
        // git@github.com:jordilin/gitar.git
        2 => {
            let domain: Vec<&str> = fields[0].split('@').collect();
            if domain.len() == 2 {
                let remote_path_partial: Vec<&str> = fields[1].split(".git").collect();
                return Ok(CmdInfo::RemoteUrl {
                    domain: domain[1].to_string(),
                    path: remote_path_partial[0].to_string(),
                });
            }
            // https://github.com/jordilin/gitar.git
            let remote_path_partial = fields[1].split('/').skip(2).collect::<Vec<&str>>();
            let host = remote_path_partial[0];
            let project = remote_path_partial[2].split(".git").collect::<Vec<&str>>();
            let project_path = format!("{}/{}", remote_path_partial[1], project[0]);
            Ok(CmdInfo::RemoteUrl {
                domain: host.to_string(),
                path: project_path,
            })
        }
        // ssh://git@gitlab-web:2000/jordilin/gitar.git
        3 => {
            let domain: Vec<&str> = fields[1].split('@').collect();
            let remote_path_partial = fields[2].split('/').skip(1).collect::<Vec<&str>>();
            let remote_path = remote_path_partial
                .join("/")
                .strip_suffix(".git")
                .unwrap() // TODO handle this?
                .to_string();
            Ok(CmdInfo::RemoteUrl {
                domain: domain[1].to_string(),
                path: remote_path,
            })
        }
        _ => {
            let trace = format!("git configuration error: {}", response.body);
            Err(error::gen(trace))
        }
    }
}

/// Get the last commit summary from the local git repository.
///
/// This will be used as the default title for the merge request. Takes a
/// [`Runner`] as a parameter and the encapsulated result is a
/// [`CmdInfo::LastCommitSummary`].
pub fn commit_summary(
    runner: Arc<impl TaskRunner<Response = Response>>,
    commit: &Option<String>,
) -> Result<CmdInfo> {
    let mut cmd_params = vec!["git", "log", "--format=%s", "-n1"];
    if let Some(commit) = commit {
        cmd_params.push(commit);
    }
    let response = runner.run(cmd_params)?;
    Ok(CmdInfo::CommitSummary(response.body))
}

pub fn outgoing_commits(
    runner: &impl TaskRunner<Response = Response>,
    remote: &str,
    default_branch: &str,
) -> Result<String> {
    let cmd = vec![
        "git".to_string(),
        "log".to_string(),
        format!("{}/{}..", remote, default_branch),
        "--reverse".to_string(),
        "--pretty=format:%s - %h %d".to_string(),
    ];
    let response = runner.run(cmd)?;
    Ok(response.body)
}

pub fn push(runner: &impl TaskRunner, remote: &str, repo: &Repo) -> Result<CmdInfo> {
    let cmd = format!("git push {} {}", remote, repo.current_branch);
    let cmd_params = cmd.split(' ').collect::<Vec<&str>>();
    runner.run(cmd_params)?;
    Ok(CmdInfo::Ignore)
}

pub fn rebase(runner: &impl TaskRunner, remote_alias: &str) -> Result<CmdInfo> {
    let cmd = format!("git rebase {}", remote_alias);
    let cmd_params = cmd.split(' ').collect::<Vec<&str>>();
    runner.run(cmd_params)?;
    Ok(CmdInfo::Ignore)
}

pub fn commit_message(
    runner: Arc<impl TaskRunner<Response = Response>>,
    commit: &Option<String>,
) -> Result<CmdInfo> {
    let mut cmd_params = vec!["git", "log", "--pretty=format:%b", "-n1"];
    if let Some(commit) = commit {
        cmd_params.push(commit);
    }
    let response = runner.run(cmd_params)?;
    Ok(CmdInfo::CommitMessage(response.body))
}

pub fn checkout(runner: &impl TaskRunner<Response = Response>, branch: &str) -> Result<()> {
    let git_cmd = format!("git checkout origin/{} -b {}", branch, branch);
    let cmd_params = ["/bin/sh", "-c", &git_cmd];
    runner.run(cmd_params).err_context(format!(
        "Failed to git checkout remote branch. Command: {}",
        cmd_params.join(" ")
    ))?;
    Ok(())
}

/// Repo represents a local git repository
#[derive(Clone, Debug, Default)]
pub struct Repo {
    current_branch: String,
    dirty: bool,
    title: String,
    last_commit_message: String,
}

impl Repo {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_current_branch(&mut self, branch: &str) {
        self.current_branch = branch.to_string();
    }

    pub fn with_status(&mut self, dirty: bool) {
        self.dirty = dirty;
    }

    pub fn with_title(&mut self, title: &str) {
        self.title = title.to_string();
    }

    pub fn with_branch(&mut self, branch: &str) {
        self.current_branch = branch.to_string();
    }

    pub fn with_last_commit_message(&mut self, message: &str) {
        self.last_commit_message = message.to_string();
    }

    pub fn current_branch(&self) -> &str {
        &self.current_branch
    }

    pub fn dirty(&self) -> bool {
        self.dirty
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn last_commit_message(&self) -> &str {
        &self.last_commit_message
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::test::utils::{get_contract, ContractType, MockRunner};

    #[test]
    fn test_git_repo_has_modified_files() {
        let response = Response::builder()
            .body(get_contract(
                ContractType::Git,
                "git_status_modified_files.txt",
            ))
            .build()
            .unwrap();
        let runner = Arc::new(MockRunner::new(vec![response]));
        let cmd_info = status(runner).unwrap();
        if let CmdInfo::StatusModified(dirty) = cmd_info {
            assert_eq!(true, dirty);
        } else {
            panic!("Expected CmdInfo::StatusModified");
        }
    }

    #[test]
    fn test_git_repo_has_untracked_and_modified_files_is_modified() {
        let response = Response::builder()
            .body(get_contract(
                ContractType::Git,
                "git_status_untracked_and_modified_files.txt",
            ))
            .build()
            .unwrap();
        let runner = Arc::new(MockRunner::new(vec![response]));
        let cmd_info = status(runner).unwrap();
        if let CmdInfo::StatusModified(dirty) = cmd_info {
            assert_eq!(true, dirty);
        } else {
            panic!("Expected CmdInfo::StatusModified");
        }
    }

    #[test]
    fn test_git_status_command_is_correct() {
        let response = Response::builder().build().unwrap();
        let runner = Arc::new(MockRunner::new(vec![response]));
        status(runner.clone()).unwrap();
        // assert_eq!("git status --short", runner.cmd.borrow().as_str());
        assert_eq!("git status --short", *runner.cmd());
    }

    #[test]
    fn test_git_repo_is_clean() {
        let response = Response::builder()
            .body(get_contract(ContractType::Git, "git_status_clean_repo.txt"))
            .build()
            .unwrap();
        let runner = Arc::new(MockRunner::new(vec![response]));
        let cmd_info = status(runner).unwrap();
        if let CmdInfo::StatusModified(dirty) = cmd_info {
            assert_eq!(false, dirty);
        } else {
            panic!("Expected CmdInfo::StatusModified");
        }
    }

    #[test]
    fn test_git_repo_has_untracked_files_treats_repo_as_no_local_modifications() {
        let response = Response::builder()
            .body(get_contract(
                ContractType::Git,
                "git_status_untracked_files.txt",
            ))
            .build()
            .unwrap();
        let runner = Arc::new(MockRunner::new(vec![response]));
        let cmd_info = status(runner).unwrap();
        if let CmdInfo::StatusModified(dirty) = cmd_info {
            assert_eq!(false, dirty);
        } else {
            panic!("Expected CmdInfo::StatusModified");
        }
    }

    #[test]
    fn test_git_remote_url_cmd_is_correct() {
        let response = Response::builder()
            .body("git@github.com:jordilin/mr.git".to_string())
            .build()
            .unwrap();
        let runner = MockRunner::new(vec![response]);
        remote_url(&runner).unwrap();
        assert_eq!("git remote get-url --all origin", *runner.cmd());
    }

    #[test]
    fn test_get_remote_git_url() {
        let response = Response::builder()
            .body("git@github.com:jordilin/mr.git".to_string())
            .build()
            .unwrap();
        let runner = MockRunner::new(vec![response]);
        let cmdinfo = remote_url(&runner).unwrap();
        match cmdinfo {
            CmdInfo::RemoteUrl { domain, path } => {
                assert_eq!("github.com", domain);
                assert_eq!("jordilin/mr", path);
            }
            _ => panic!("Failed to parse remote url"),
        }
    }

    #[test]
    fn test_get_remote_https_url() {
        let response = Response::builder()
            .body("https://github.com/jordilin/gitar.git".to_string())
            .build()
            .unwrap();
        let runner = MockRunner::new(vec![response]);
        let cmdinfo = remote_url(&runner).unwrap();
        match cmdinfo {
            CmdInfo::RemoteUrl { domain, path } => {
                assert_eq!("github.com", domain);
                assert_eq!("jordilin/gitar", path);
            }
            _ => panic!("Failed to parse remote url"),
        }
    }

    #[test]
    fn test_get_remote_ssh_url() {
        let response = Response::builder()
            .body("ssh://git@gitlab-web:2222/testgroup/testsubproject.git".to_string())
            .build()
            .unwrap();
        let runner = MockRunner::new(vec![response]);
        let cmdinfo = remote_url(&runner).unwrap();
        match cmdinfo {
            CmdInfo::RemoteUrl { domain, path } => {
                assert_eq!("gitlab-web", domain);
                assert_eq!("testgroup/testsubproject", path);
            }
            _ => panic!("Failed to parse remote url"),
        }
    }

    #[test]
    fn test_remote_url_no_remote() {
        let response = Response::builder()
            .status(1)
            .body("error: No such remote 'origin'".to_string())
            .build()
            .unwrap();
        let runner = MockRunner::new(vec![response]);
        assert!(remote_url(&runner).is_err())
    }

    #[test]
    fn test_empty_remote_url() {
        let response = Response::builder().build().unwrap();
        let runner = MockRunner::new(vec![response]);
        assert!(remote_url(&runner).is_err())
    }

    #[test]
    fn test_git_fetch_cmd_is_correct() {
        let response = Response::builder().build().unwrap();
        let runner = Arc::new(MockRunner::new(vec![response]));
        fetch(runner.clone(), "origin".to_string()).unwrap();
        assert_eq!("git fetch origin", *runner.cmd());
    }

    #[test]
    fn test_gather_current_branch_cmd_is_correct() {
        let response = Response::builder().build().unwrap();
        let runner = Arc::new(MockRunner::new(vec![response]));
        current_branch(runner.clone()).unwrap();
        assert_eq!("git rev-parse --abbrev-ref HEAD", *runner.cmd());
    }

    #[test]
    fn test_gather_current_branch_ok() {
        let response = Response::builder()
            .body(get_contract(ContractType::Git, "git_current_branch.txt"))
            .build()
            .unwrap();
        let runner = Arc::new(MockRunner::new(vec![response]));
        let cmdinfo = current_branch(runner).unwrap();
        if let CmdInfo::Branch(branch) = cmdinfo {
            assert_eq!("main", branch);
        } else {
            panic!("Expected CmdInfo::Branch");
        }
    }

    #[test]
    fn test_last_commit_summary_cmd_is_correct() {
        let response = Response::builder()
            .body("Add README".to_string())
            .build()
            .unwrap();
        let runner = Arc::new(MockRunner::new(vec![response]));
        commit_summary(runner.clone(), &None).unwrap();
        assert_eq!("git log --format=%s -n1", *runner.cmd());
    }

    #[test]
    fn test_last_commit_summary_get_last_commit() {
        let response = Response::builder()
            .body("Add README".to_string())
            .build()
            .unwrap();
        let runner = MockRunner::new(vec![response]);
        let title = commit_summary(Arc::new(runner), &None).unwrap();
        if let CmdInfo::CommitSummary(title) = title {
            assert_eq!("Add README", title);
        } else {
            panic!("Expected CmdInfo::LastCommitSummary");
        }
    }

    #[test]
    fn test_last_commit_summary_errors() {
        let response = Response::builder()
            .status(1)
            .body("Could not retrieve last commit".to_string())
            .build()
            .unwrap();
        let runner = Arc::new(MockRunner::new(vec![response]));
        assert!(commit_summary(runner, &None).is_err());
    }

    #[test]
    fn test_commit_summary_specific_sha_cmd_is_correct() {
        let response = Response::builder()
            .body("Add README".to_string())
            .build()
            .unwrap();
        let runner = Arc::new(MockRunner::new(vec![response]));
        commit_summary(runner.clone(), &Some("123456".to_string())).unwrap();
        assert_eq!("git log --format=%s -n1 123456", *runner.cmd());
    }

    #[test]
    fn test_git_push_cmd_is_correct() {
        let response = Response::builder().build().unwrap();
        let runner = MockRunner::new(vec![response]);
        let mut repo = Repo::new();
        repo.with_current_branch("new_feature");
        push(&runner, "origin", &repo).unwrap();
        assert_eq!("git push origin new_feature", *runner.cmd());
    }

    #[test]
    fn test_git_push_cmd_fails() {
        let response = Response::builder()
            .status(1)
            .body(get_contract(ContractType::Git, "git_push_failure.txt"))
            .build()
            .unwrap();
        let runner = MockRunner::new(vec![response]);
        let mut repo = Repo::new();
        repo.with_current_branch("new_feature");
        assert!(push(&runner, "origin", &repo).is_err());
    }

    #[test]
    fn test_repo_is_dirty_if_there_are_local_changes() {
        let mut repo = Repo::new();
        repo.with_status(true);
        assert!(repo.dirty())
    }

    #[test]
    fn test_repo_title_based_on_cmdinfo_lastcommit_summary() {
        let mut repo = Repo::new();
        repo.with_title("Add README");
        assert_eq!(repo.title(), "Add README")
    }

    #[test]
    fn test_repo_current_branch_based_on_cmdinfo_branch() {
        let mut repo = Repo::new();
        repo.with_current_branch("new_feature");
        assert_eq!(repo.current_branch(), "new_feature")
    }

    #[test]
    fn test_git_rebase_cmd_is_correct() {
        let response = Response::builder().build().unwrap();
        let runner = MockRunner::new(vec![response]);
        rebase(&runner, "origin/main").unwrap();
        assert_eq!("git rebase origin/main", *runner.cmd());
    }

    #[test]
    fn test_git_rebase_fails_throws_error() {
        let response = Response::builder()
            .status(1)
            .body(get_contract(
                ContractType::Git,
                "git_rebase_wrong_origin.txt",
            ))
            .build()
            .unwrap();
        let runner = MockRunner::new(vec![response]);
        assert!(rebase(&runner, "origin/main").is_err())
    }

    #[test]
    fn test_outgoing_commits_cmd_is_ok() {
        let response = Response::builder().build().unwrap();
        let runner = MockRunner::new(vec![response]);
        outgoing_commits(&runner, "origin", "main").unwrap();
        let expected_cmd = "git log origin/main.. --reverse --pretty=format:%s - %h %d".to_string();
        assert_eq!(expected_cmd, *runner.cmd());
    }

    #[test]
    fn test_last_commit_message_cmd_is_ok() {
        let response = Response::builder().build().unwrap();
        let runner = Arc::new(MockRunner::new(vec![response]));
        commit_message(runner.clone(), &None).unwrap();
        let expected_cmd = "git log --pretty=format:%b -n1".to_string();
        assert_eq!(expected_cmd, *runner.cmd());
    }

    #[test]
    fn test_commit_message_from_specific_commit_cmd_is_ok() {
        let response = Response::builder().build().unwrap();
        let runner = Arc::new(MockRunner::new(vec![response]));
        commit_message(runner.clone(), &Some("123456".to_string())).unwrap();
        let expected_cmd = "git log --pretty=format:%b -n1 123456".to_string();
        assert_eq!(expected_cmd, *runner.cmd());
    }

    #[test]
    fn test_git_add_changes_cmd_is_ok() {
        let response = Response::builder().build().unwrap();
        let runner = MockRunner::new(vec![response]);
        add(&runner).unwrap();
        let expected_cmd = "git add -u".to_string();
        assert_eq!(expected_cmd, *runner.cmd());
    }

    #[test]
    fn test_git_add_changes_cmd_is_err() {
        let response = Response::builder()
            .status(1)
            .body("error: could not add changes".to_string())
            .build()
            .unwrap();
        let runner = MockRunner::new(vec![response]);
        assert!(add(&runner).is_err());
    }

    #[test]
    fn test_git_commit_message_is_ok() {
        let response = Response::builder()
            .body("Add README".to_string())
            .build()
            .unwrap();
        let runner = MockRunner::new(vec![response]);
        commit(&runner, "Add README").unwrap();
        let expected_cmd = "git commit -m Add README".to_string();
        assert_eq!(expected_cmd, *runner.cmd());
    }

    #[test]
    fn test_git_commit_message_is_err() {
        let response = Response::builder()
            .status(1)
            .body("error: could not commit changes".to_string())
            .build()
            .unwrap();
        let runner = MockRunner::new(vec![response]);
        assert!(commit(&runner, "Add README").is_err());
    }
}
