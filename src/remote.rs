use std::fmt::{self, Display, Formatter};

use crate::api_traits::Remote;
use crate::cache::filesystem::FileCache;
use crate::config::Config;
use crate::github::Github;
use crate::gitlab::Gitlab;
use crate::Result;
use crate::{error, http};
use std::sync::Arc;

#[derive(Debug, Default, PartialEq)]
pub struct Project {
    id: i64,
    default_branch: String,
    members: Vec<Member>,
}

impl Project {
    pub fn new(id: i64, default_branch: &str) -> Self {
        Project {
            id,
            default_branch: default_branch.to_string(),
            members: Vec::new(),
        }
    }

    pub fn default_branch(&self) -> &str {
        &self.default_branch
    }
}

#[derive(Debug, PartialEq, Default)]
pub struct Member {
    pub id: i64,
    pub name: String,
    pub username: String,
}

impl Member {
    pub fn new(id: i64, name: &str, username: &str) -> Self {
        Member {
            id,
            name: name.to_string(),
            username: username.to_string(),
        }
    }
}

#[derive(Debug)]
pub struct MergeRequestResponse {
    pub id: i64,
    pub web_url: String,
    pub author: String,
    pub updated_at: String,
    pub source_branch: String,
}

impl MergeRequestResponse {
    pub fn new(
        id: i64,
        web_url: &str,
        author: &str,
        updated_at: &str,
        source_branch: &str,
    ) -> Self {
        MergeRequestResponse {
            id,
            web_url: web_url.to_string(),
            author: author.to_string(),
            updated_at: updated_at.to_string(),
            source_branch: source_branch.to_string(),
        }
    }
}

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

pub struct MergeRequestArgs {
    title: String,
    description: String,
    source_branch: String,
    target_branch: String,
    assignee_id: String,
    username: String,
    remove_source_branch: String,
}

impl Default for MergeRequestArgs {
    fn default() -> Self {
        MergeRequestArgs {
            title: String::new(),
            description: String::new(),
            source_branch: String::new(),
            target_branch: String::new(),
            assignee_id: String::new(),
            username: String::new(),
            remove_source_branch: "true".to_string(),
        }
    }
}

impl MergeRequestArgs {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_title(mut self, title: &str) -> Self {
        self.title = title.to_string();
        self
    }

    pub fn with_description(mut self, description: &str) -> Self {
        self.description = description.to_string();
        self
    }

    pub fn with_source_branch(mut self, source_branch: &str) -> Self {
        self.source_branch = source_branch.to_string();
        self
    }

    pub fn with_target_branch(mut self, target_branch: &str) -> Self {
        self.target_branch = target_branch.to_string();
        self
    }

    pub fn with_assignee_id(mut self, assignee_id: &str) -> Self {
        self.assignee_id = assignee_id.to_string();
        self
    }

    pub fn with_username(mut self, username: &str) -> Self {
        self.username = username.to_string();
        self
    }

    pub fn with_remove_source_branch(mut self, remove_source_branch: bool) -> Self {
        self.remove_source_branch = remove_source_branch.to_string();
        self
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    pub fn source_branch(&self) -> &str {
        &self.source_branch
    }

    pub fn target_branch(&self) -> &str {
        &self.target_branch
    }

    pub fn assignee_id(&self) -> &str {
        &self.assignee_id
    }

    pub fn username(&self) -> &str {
        &self.username
    }

    pub fn remove_source_branch(&self) -> &str {
        &self.remove_source_branch
    }
}

#[derive(Debug)]
pub struct Pipeline {
    status: String,
    web_url: String,
    branch: String,
    sha: String,
    created_at: String,
}

impl Pipeline {
    pub fn new(status: &str, web_url: &str, branch: &str, sha: &str, created_at: &str) -> Self {
        Pipeline {
            status: status.to_string(),
            web_url: web_url.to_string(),
            branch: branch.to_string(),
            sha: sha.to_string(),
            created_at: created_at.to_string(),
        }
    }
}

impl Display for Pipeline {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} | {} | {} | {} | {}",
            self.web_url, self.branch, self.sha, self.created_at, self.status
        )
    }
}

pub fn get(
    domain: String,
    path: String,
    config: Config,
    refresh_cache: bool,
) -> Result<Arc<dyn Remote>> {
    let runner = Arc::new(http::Client::new(
        FileCache::new(config.clone()),
        refresh_cache,
    ));
    let github_domain_regex = regex::Regex::new(r"^github").unwrap();
    let gitlab_domain_regex = regex::Regex::new(r"^gitlab").unwrap();

    let remote: Arc<dyn Remote> = if github_domain_regex.is_match(&domain) {
        Arc::new(Github::new(config, &domain, &path, runner))
    } else if gitlab_domain_regex.is_match(&domain) {
        Arc::new(Gitlab::new(config, &domain, &path, runner))
    } else {
        return Err(error::gen(format!("Unsupported domain: {}", &domain)));
    };
    Ok(remote)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_merge_request_args_with_custom_title() {
        let args = MergeRequestArgs::new()
            .with_source_branch("source")
            .with_target_branch("target")
            .with_title("title");

        assert_eq!(args.source_branch, "source");
        assert_eq!(args.target_branch, "target");
        assert_eq!(args.title, "title");
        assert_eq!(args.remove_source_branch, "true");
        assert_eq!(args.description, "");
    }

    #[test]
    fn test_merge_request_get_all_fields() {
        let args = MergeRequestArgs::new()
            .with_source_branch("source")
            .with_target_branch("target")
            .with_title("title")
            .with_description("description")
            .with_assignee_id("assignee_id")
            .with_username("username")
            .with_remove_source_branch(false);

        assert_eq!(args.source_branch, "source");
        assert_eq!(args.target_branch, "target");
        assert_eq!(args.title, "title");
        assert_eq!(args.description, "description");
        assert_eq!(args.assignee_id, "assignee_id");
        assert_eq!(args.username, "username");
        assert_eq!(args.remove_source_branch, "false");
    }
}
