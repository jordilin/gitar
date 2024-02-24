use std::fmt::{self, Display, Formatter};

use crate::api_traits::{Cicd, MergeRequest, RemoteProject, Timestamp};
use crate::cache::filesystem::FileCache;
use crate::config::Config;
use crate::error::GRError;
use crate::github::Github;
use crate::gitlab::Gitlab;
use crate::Result;
use crate::{error, http};
use std::convert::TryFrom;
use std::sync::Arc;

pub mod query;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Project {
    id: i64,
    default_branch: String,
    members: Vec<Member>,
    html_url: String,
    created_at: String,
}

impl Project {
    pub fn new(id: i64, default_branch: &str) -> Self {
        Project {
            id,
            default_branch: default_branch.to_string(),
            members: Vec::new(),
            html_url: String::new(),
            created_at: String::new(),
        }
    }

    pub fn with_html_url(mut self, html_url: &str) -> Self {
        self.html_url = html_url.to_string();
        self
    }

    // TODO - builder pattern
    pub fn with_created_at(mut self, created_at: &str) -> Self {
        self.created_at = created_at.to_string();
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

impl Timestamp for Project {
    fn created_at(&self) -> String {
        self.created_at.clone()
    }
}

#[derive(Builder, Clone, Debug, PartialEq, Default)]
pub struct Member {
    pub id: i64,
    pub name: String,
    pub username: String,
    #[builder(default)]
    pub created_at: String,
}

impl Member {
    pub fn builder() -> MemberBuilder {
        MemberBuilder::default()
    }
}

impl Timestamp for Member {
    fn created_at(&self) -> String {
        self.created_at.clone()
    }
}

#[derive(Builder, Clone, Debug)]
pub struct MergeRequestResponse {
    #[builder(default)]
    pub id: i64,
    #[builder(default)]
    pub web_url: String,
    #[builder(default)]
    pub author: String,
    #[builder(default)]
    pub updated_at: String,
    #[builder(default)]
    pub source_branch: String,
    #[builder(default)]
    pub created_at: String,
}

impl MergeRequestResponse {
    pub fn builder() -> MergeRequestResponseBuilder {
        MergeRequestResponseBuilder::default()
    }
}

impl Timestamp for MergeRequestResponse {
    fn created_at(&self) -> String {
        self.created_at.clone()
    }
}

#[derive(Clone, Copy)]
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

#[derive(Builder, Clone)]
pub struct MergeRequestListBodyArgs {
    pub state: MergeRequestState,
    pub list_args: Option<ListBodyArgs>,
}

impl MergeRequestListBodyArgs {
    pub fn builder() -> MergeRequestListBodyArgsBuilder {
        MergeRequestListBodyArgsBuilder::default()
    }
}

#[derive(Builder, Clone, Debug)]
pub struct Pipeline {
    pub status: String,
    web_url: String,
    branch: String,
    sha: String,
    created_at: String,
}

impl Pipeline {
    pub fn builder() -> PipelineBuilder {
        PipelineBuilder::default()
    }
}

impl Timestamp for Pipeline {
    fn created_at(&self) -> String {
        self.created_at.clone()
    }
}

/// List cli args can be used across multiple APIs that support pagination.
#[derive(Builder)]
pub struct ListRemoteCliArgs {
    #[builder(default)]
    pub from_page: Option<i64>,
    #[builder(default)]
    pub to_page: Option<i64>,
    #[builder(default)]
    pub num_pages: bool,
    #[builder(default)]
    pub refresh_cache: bool,
    #[builder(default)]
    pub no_headers: bool,
    #[builder(default)]
    pub page_number: Option<i64>,
    #[builder(default)]
    pub created_after: Option<String>,
    #[builder(default)]
    pub sort: ListSortMode,
}

impl ListRemoteCliArgs {
    pub fn builder() -> ListRemoteCliArgsBuilder {
        ListRemoteCliArgsBuilder::default()
    }
}

/// List body args is a common structure that can be used across multiple APIs
/// that support pagination. `list` operations in traits that accept some sort
/// of List related arguments can encapsulate this structure. Example of those
/// is `MergeRequestListBodyArgs`. This can be consumed by Github and Gitlab
/// clients when executing HTTP requests.
#[derive(Builder, Clone)]
pub struct ListBodyArgs {
    #[builder(setter(strip_option), default)]
    pub page: Option<i64>,
    #[builder(setter(strip_option), default)]
    pub max_pages: Option<i64>,
    #[builder(default)]
    pub created_after: Option<String>,
    #[builder(default)]
    pub sort_mode: ListSortMode,
}

impl ListBodyArgs {
    pub fn builder() -> ListBodyArgsBuilder {
        ListBodyArgsBuilder::default()
    }
}

pub fn validate_from_to_page(remote_cli_args: &ListRemoteCliArgs) -> Result<Option<ListBodyArgs>> {
    if remote_cli_args.page_number.is_some() {
        return Ok(Some(
            ListBodyArgs::builder()
                .page(remote_cli_args.page_number.unwrap())
                .max_pages(1)
                .sort_mode(remote_cli_args.sort.clone())
                .created_after(remote_cli_args.created_after.clone())
                .build()
                .unwrap(),
        ));
    }
    let body_args = match (remote_cli_args.from_page, remote_cli_args.to_page) {
        (Some(from_page), Some(to_page)) => {
            if from_page < 0 || to_page < 0 {
                return Err(GRError::PreconditionNotMet(
                    "from_page and to_page must be a positive number".to_string(),
                )
                .into());
            }
            if from_page >= to_page {
                return Err(GRError::PreconditionNotMet(
                    "from_page must be less than to_page".to_string(),
                )
                .into());
            }

            let max_pages = to_page - from_page + 1;
            Some(
                ListBodyArgs::builder()
                    .page(from_page)
                    .max_pages(max_pages)
                    .sort_mode(remote_cli_args.sort.clone())
                    .build()
                    .unwrap(),
            )
        }
        (Some(_), None) => {
            return Err(
                GRError::PreconditionNotMet("from_page requires the to_page".to_string()).into(),
            );
        }
        (None, Some(to_page)) => {
            if to_page < 0 {
                return Err(GRError::PreconditionNotMet(
                    "to_page must be a positive number".to_string(),
                )
                .into());
            }
            Some(
                ListBodyArgs::builder()
                    .page(1)
                    .max_pages(to_page)
                    .sort_mode(remote_cli_args.sort.clone())
                    .build()
                    .unwrap(),
            )
        }
        (None, None) => None,
    };
    if let Some(created_after) = &remote_cli_args.created_after {
        if let Some(body_args) = &body_args {
            return Ok(Some(
                ListBodyArgs::builder()
                    .page(body_args.page.unwrap())
                    .max_pages(body_args.max_pages.unwrap())
                    .created_after(Some(created_after.to_string()))
                    .sort_mode(remote_cli_args.sort.clone())
                    .build()
                    .unwrap(),
            ));
        }
        return Ok(Some(
            ListBodyArgs::builder()
                .created_after(Some(created_after.to_string()))
                .sort_mode(remote_cli_args.sort.clone())
                .build()
                .unwrap(),
        ));
    }
    Ok(body_args)
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum ListSortMode {
    #[default]
    Asc,
    Desc,
}

#[derive(Builder, Clone)]
pub struct PipelineBodyArgs {
    pub from_to_page: Option<ListBodyArgs>,
}

impl PipelineBodyArgs {
    pub fn builder() -> PipelineBodyArgsBuilder {
        PipelineBodyArgsBuilder::default()
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

    #[test]
    fn test_cli_from_to_pages_valid_range() {
        let from_page = Option::Some(1);
        let to_page = Option::Some(3);
        let args = ListRemoteCliArgs::builder()
            .from_page(from_page)
            .to_page(to_page)
            .build()
            .unwrap();
        let args = validate_from_to_page(&args).unwrap().unwrap();
        assert_eq!(args.page, Some(1));
        assert_eq!(args.max_pages, Some(3));
    }

    #[test]
    fn test_cli_from_to_pages_invalid_range() {
        let from_page = Some(5);
        let to_page = Some(2);
        let args = ListRemoteCliArgs::builder()
            .from_page(from_page)
            .to_page(to_page)
            .build()
            .unwrap();
        let args = validate_from_to_page(&args);
        match args {
            Err(err) => match err.downcast_ref::<error::GRError>() {
                Some(error::GRError::PreconditionNotMet(_)) => (),
                _ => panic!("Expected error::GRError::PreconditionNotMet"),
            },
            _ => panic!("Expected error"),
        }
    }

    #[test]
    fn test_cli_from_page_negative_number_is_error() {
        let from_page = Some(-5);
        let to_page = Some(5);
        let args = ListRemoteCliArgs::builder()
            .from_page(from_page)
            .to_page(to_page)
            .build()
            .unwrap();
        let args = validate_from_to_page(&args);
        match args {
            Err(err) => match err.downcast_ref::<error::GRError>() {
                Some(error::GRError::PreconditionNotMet(_)) => (),
                _ => panic!("Expected error::GRError::PreconditionNotMet"),
            },
            _ => panic!("Expected error"),
        }
    }

    #[test]
    fn test_cli_to_page_negative_number_is_error() {
        let from_page = Some(5);
        let to_page = Some(-5);
        let args = ListRemoteCliArgs::builder()
            .from_page(from_page)
            .to_page(to_page)
            .build()
            .unwrap();
        let args = validate_from_to_page(&args);
        match args {
            Err(err) => match err.downcast_ref::<error::GRError>() {
                Some(error::GRError::PreconditionNotMet(_)) => (),
                _ => panic!("Expected error::GRError::PreconditionNotMet"),
            },
            _ => panic!("Expected error"),
        }
    }

    #[test]
    fn test_cli_from_page_without_to_page_is_error() {
        let from_page = Some(5);
        let to_page = None;
        let args = ListRemoteCliArgs::builder()
            .from_page(from_page)
            .to_page(to_page)
            .build()
            .unwrap();
        let args = validate_from_to_page(&args);
        match args {
            Err(err) => match err.downcast_ref::<error::GRError>() {
                Some(error::GRError::PreconditionNotMet(_)) => (),
                _ => panic!("Expected error::GRError::PreconditionNotMet"),
            },
            _ => panic!("Expected error"),
        }
    }

    #[test]
    fn test_if_from_and_to_provided_must_be_positive() {
        let from_page = Some(-5);
        let to_page = Some(-5);
        let args = ListRemoteCliArgs::builder()
            .from_page(from_page)
            .to_page(to_page)
            .build()
            .unwrap();
        let args = validate_from_to_page(&args);
        match args {
            Err(err) => match err.downcast_ref::<error::GRError>() {
                Some(error::GRError::PreconditionNotMet(_)) => (),
                _ => panic!("Expected error::GRError::PreconditionNotMet"),
            },
            _ => panic!("Expected error"),
        }
    }

    #[test]
    fn test_if_page_number_provided_max_pages_is_1() {
        let page_number = Some(5);
        let args = ListRemoteCliArgs::builder()
            .page_number(page_number)
            .build()
            .unwrap();
        let args = validate_from_to_page(&args).unwrap().unwrap();
        assert_eq!(args.page, Some(5));
        assert_eq!(args.max_pages, Some(1));
    }

    #[test]
    fn test_include_created_after_in_list_body_args() {
        let created_after = "2021-01-01T00:00:00Z";
        let args = ListRemoteCliArgs::builder()
            .created_after(Some(created_after.to_string()))
            .build()
            .unwrap();
        let args = validate_from_to_page(&args).unwrap().unwrap();
        assert_eq!(args.created_after.unwrap(), created_after);
    }

    #[test]
    fn test_includes_from_to_page_and_created_after_in_list_body_args() {
        let from_page = Some(1);
        let to_page = Some(3);
        let created_after = "2021-01-01T00:00:00Z";
        let args = ListRemoteCliArgs::builder()
            .from_page(from_page)
            .to_page(to_page)
            .created_after(Some(created_after.to_string()))
            .build()
            .unwrap();
        let args = validate_from_to_page(&args).unwrap().unwrap();
        assert_eq!(args.page, Some(1));
        assert_eq!(args.max_pages, Some(3));
        assert_eq!(args.created_after.unwrap(), created_after);
    }

    #[test]
    fn test_includes_sort_mode_in_list_body_args_used_with_created_after() {
        let args = ListRemoteCliArgs::builder()
            .created_after(Some("2021-01-01T00:00:00Z".to_string()))
            .sort(ListSortMode::Desc)
            .build()
            .unwrap();
        let args = validate_from_to_page(&args).unwrap().unwrap();
        assert_eq!(args.sort_mode, ListSortMode::Desc);
    }

    #[test]
    fn test_includes_sort_mode_in_list_body_args_used_with_from_to_page() {
        let from_page = Some(1);
        let to_page = Some(3);
        let args = ListRemoteCliArgs::builder()
            .from_page(from_page)
            .to_page(to_page)
            .sort(ListSortMode::Desc)
            .build()
            .unwrap();
        let args = validate_from_to_page(&args).unwrap().unwrap();
        assert_eq!(args.sort_mode, ListSortMode::Desc);
    }

    #[test]
    fn test_includes_sort_mode_in_list_body_args_used_with_page_number() {
        let page_number = Some(1);
        let args = ListRemoteCliArgs::builder()
            .page_number(page_number)
            .sort(ListSortMode::Desc)
            .build()
            .unwrap();
        let args = validate_from_to_page(&args).unwrap().unwrap();
        assert_eq!(args.sort_mode, ListSortMode::Desc);
    }

    #[test]
    fn test_add_created_after_with_page_number() {
        let page_number = Some(1);
        let created_after = "2021-01-01T00:00:00Z";
        let args = ListRemoteCliArgs::builder()
            .page_number(page_number)
            .created_after(Some(created_after.to_string()))
            .build()
            .unwrap();
        let args = validate_from_to_page(&args).unwrap().unwrap();
        assert_eq!(args.page.unwrap(), 1);
        assert_eq!(args.max_pages.unwrap(), 1);
        assert_eq!(args.created_after.unwrap(), created_after);
    }
}
