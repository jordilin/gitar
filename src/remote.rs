use std::fmt::{self, Display, Formatter};

use crate::api_traits::{Cicd, MergeRequest, RemoteProject};
use crate::cache::filesystem::FileCache;
use crate::config::Config;
use crate::github::Github;
use crate::gitlab::Gitlab;
use crate::Result;
use crate::{error, http};
use std::convert::TryFrom;
use std::sync::Arc;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Project {
    id: i64,
    default_branch: String,
    members: Vec<Member>,
    html_url: String,
}

impl Project {
    pub fn new(id: i64, default_branch: &str) -> Self {
        Project {
            id,
            default_branch: default_branch.to_string(),
            members: Vec::new(),
            html_url: String::new(),
        }
    }

    pub fn with_html_url(mut self, html_url: &str) -> Self {
        self.html_url = html_url.to_string();
        self
    }

    pub fn default_branch(&self) -> &str {
        &self.default_branch
    }
}

impl Display for Project {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        writeln!(f, "ID | Default Branch | URL")?;
        write!(
            f,
            "{} | {} | {} ",
            self.id, self.default_branch, self.html_url
        )
    }
}

#[derive(Builder, Clone, Debug, PartialEq, Default)]
pub struct Member {
    pub id: i64,
    pub name: String,
    pub username: String,
}

impl Member {
    pub fn builder() -> MemberBuilder {
        MemberBuilder::default()
    }
}

#[derive(Clone, Debug)]
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

#[derive(Builder)]
pub struct MergeRequestBodyArgs {
    #[builder(default)]
    pub title: String,
    #[builder(default)]
    pub description: String,
    #[builder(default)]
    pub source_branch: String,
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
}

impl MergeRequestBodyArgs {
    pub fn builder() -> MergeRequestBodyArgsBuilder {
        MergeRequestBodyArgsBuilder::default()
    }
}

#[derive(Clone, Debug)]
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

macro_rules! get {
    ($func_name:ident, $trait_name:ident) => {
        pub fn $func_name(
            domain: String,
            path: String,
            config: Arc<Config>,
            refresh_cache: bool,
        ) -> Result<Arc<dyn $trait_name + Send + Sync + 'static>> {
            let runner = Arc::new(http::Client::new(
                FileCache::new(config.clone()),
                config.clone(),
                refresh_cache,
            ));

            let github_domain_regex = regex::Regex::new(r"^github").unwrap();
            let gitlab_domain_regex = regex::Regex::new(r"^gitlab").unwrap();
            let remote: Arc<dyn $trait_name + Send + Sync + 'static> =
                if github_domain_regex.is_match(&domain) {
                    Arc::new(Github::new(config, &domain, &path, runner))
                } else if gitlab_domain_regex.is_match(&domain) {
                    Arc::new(Gitlab::new(config, &domain, &path, runner))
                } else {
                    return Err(error::gen(format!("Unsupported domain: {}", &domain)));
                };
            Ok(remote)
        }
    };
}

get!(get_mr, MergeRequest);
get!(get_cicd, Cicd);
get!(get_project, RemoteProject);

#[cfg(test)]
mod test {
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
}
