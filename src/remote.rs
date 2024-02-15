use std::fmt::{self, Display, Formatter};

use crate::api_traits::{ApiOperation, Cicd, MergeRequest, RemoteProject};
use crate::cache::filesystem::FileCache;
use crate::config::Config;
use crate::error::GRError;
use crate::github::Github;
use crate::gitlab::Gitlab;
use crate::http::Request;
use crate::io::{HttpRunner, Response};
use crate::Result;
use crate::{error, http};
use std::convert::TryFrom;
use std::sync::Arc;

pub struct Token {
    header_name: String,
    value: String,
}

impl Token {
    pub fn new(header_name: &str, value: &str) -> Self {
        Token {
            header_name: header_name.to_string(),
            value: value.to_string(),
        }
    }
}

pub fn num_pages<R: HttpRunner<Response = Response>>(
    runner: &Arc<R>,
    url: &str,
    token: Token,
    operation: ApiOperation,
) -> Result<Option<u32>> {
    let mut request: Request<()> =
        http::Request::new(&url, http::Method::GET).with_api_operation(operation);
    request.set_header(&token.header_name, &token.value);
    let response = runner.run(&mut request)?;
    let page_header = response
        .get_page_headers()
        .ok_or_else(|| error::gen(format!("Failed to get page headers for URL: {}", url)))?;
    if let Some(last_page) = page_header.last {
        return Ok(Some(last_page.number));
    }
    Ok(None)
}

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
}

impl MergeRequestResponse {
    pub fn builder() -> MergeRequestResponseBuilder {
        MergeRequestResponseBuilder::default()
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
    pub page: i64,
    pub max_pages: i64,
}

impl ListBodyArgs {
    pub fn builder() -> ListBodyArgsBuilder {
        ListBodyArgsBuilder::default()
    }
}

pub fn validate_from_to_page(remote_cli_args: &ListRemoteCliArgs) -> Result<Option<ListBodyArgs>> {
    return match (remote_cli_args.from_page, remote_cli_args.to_page) {
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
            Ok(Some(
                ListBodyArgs::builder()
                    .page(from_page)
                    .max_pages(max_pages)
                    .build()
                    .unwrap(),
            ))
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
            Ok(Some(
                ListBodyArgs::builder()
                    .page(1)
                    .max_pages(to_page)
                    .build()
                    .unwrap(),
            ))
        }
        (None, None) => Ok(None),
    };
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
        assert_eq!(args.page, 1);
        assert_eq!(args.max_pages, 3);
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
}
