use std::fmt::{self, Display, Formatter};
use std::fs::File;
use std::path::Path;

use crate::api_traits::{
    Cicd, CicdJob, CicdRunner, CodeGist, CommentMergeRequest, ContainerRegistry, Deploy,
    DeployAsset, MergeRequest, RemoteProject, Timestamp, TrendingProjectURL, UserInfo,
};
use crate::cache::{filesystem::FileCache, nocache::NoCache};
use crate::config::Config;
use crate::display::{Column, DisplayBody, Format};
use crate::error::GRError;
use crate::github::Github;
use crate::gitlab::Gitlab;
use crate::io::{CmdInfo, HttpRunner, Response, TaskRunner};
use crate::time::Milliseconds;
use crate::{cli, error, http};
use crate::{git, Result};
use std::convert::TryFrom;
use std::sync::Arc;

pub mod query;

#[derive(Builder, Clone, Debug, Default, PartialEq)]
pub struct Project {
    pub id: i64,
    default_branch: String,
    #[builder(default)]
    members: Vec<Member>,
    html_url: String,
    created_at: String,
    description: String,
    // Field not available in Gitlab. Set to empty string.
    #[builder(default)]
    language: String,
}

impl Project {
    pub fn builder() -> ProjectBuilder {
        ProjectBuilder::default()
    }

    pub fn new(id: i64, default_branch: &str) -> Self {
        Project {
            id,
            default_branch: default_branch.to_string(),
            members: Vec::new(),
            html_url: String::new(),
            created_at: String::new(),
            description: String::new(),
            language: String::new(),
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

impl From<Project> for DisplayBody {
    fn from(p: Project) -> DisplayBody {
        DisplayBody {
            columns: vec![
                Column::new("ID", p.id.to_string()),
                Column::new("Default Branch", p.default_branch),
                Column::new("URL", p.html_url),
                Column::new("Created at", p.created_at),
                Column::builder()
                    .name("Description".to_string())
                    .value(p.description)
                    .optional(true)
                    .build()
                    .unwrap(),
                Column::builder()
                    .name("Language".to_string())
                    .value(p.language)
                    .optional(true)
                    .build()
                    .unwrap(),
            ],
        }
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

impl From<Member> for DisplayBody {
    fn from(m: Member) -> DisplayBody {
        DisplayBody {
            columns: vec![
                Column::new("ID", m.id.to_string()),
                Column::new("Name", m.name),
                Column::new("Username", m.username),
            ],
        }
    }
}

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

/// List cli args can be used across multiple APIs that support pagination.
#[derive(Builder, Clone)]
pub struct ListRemoteCliArgs {
    #[builder(default)]
    pub from_page: Option<i64>,
    #[builder(default)]
    pub to_page: Option<i64>,
    #[builder(default)]
    pub num_pages: bool,
    #[builder(default)]
    pub num_resources: bool,
    #[builder(default)]
    pub page_number: Option<i64>,
    #[builder(default)]
    pub created_after: Option<String>,
    #[builder(default)]
    pub created_before: Option<String>,
    #[builder(default)]
    pub sort: ListSortMode,
    #[builder(default)]
    pub flush: bool,
    #[builder(default)]
    pub throttle_time: Option<Milliseconds>,
    #[builder(default)]
    pub throttle_range: Option<(Milliseconds, Milliseconds)>,
    #[builder(default)]
    pub get_args: GetRemoteCliArgs,
}

impl ListRemoteCliArgs {
    pub fn builder() -> ListRemoteCliArgsBuilder {
        ListRemoteCliArgsBuilder::default()
    }
}

#[derive(Builder, Clone, Default)]
pub struct GetRemoteCliArgs {
    #[builder(default)]
    pub no_headers: bool,
    #[builder(default)]
    pub format: Format,
    #[builder(default)]
    pub cache_args: CacheCliArgs,
    #[builder(default)]
    pub display_optional: bool,
    #[builder(default)]
    pub backoff_max_retries: u32,
    #[builder(default)]
    pub backoff_retry_after: u64,
}

impl GetRemoteCliArgs {
    pub fn builder() -> GetRemoteCliArgsBuilder {
        GetRemoteCliArgsBuilder::default()
    }
}

#[derive(Builder, Clone, Default)]
pub struct CacheCliArgs {
    #[builder(default)]
    pub refresh: bool,
    #[builder(default)]
    pub no_cache: bool,
}

impl CacheCliArgs {
    pub fn builder() -> CacheCliArgsBuilder {
        CacheCliArgsBuilder::default()
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
    pub created_before: Option<String>,
    #[builder(default)]
    pub sort_mode: ListSortMode,
    #[builder(default)]
    pub flush: bool,
    #[builder(default)]
    pub throttle_time: Option<Milliseconds>,
    #[builder(default)]
    pub throttle_range: Option<(Milliseconds, Milliseconds)>,
    // Carry display format for flush operations
    #[builder(default)]
    pub get_args: GetRemoteCliArgs,
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
                .created_before(remote_cli_args.created_before.clone())
                .build()
                .unwrap(),
        ));
    }
    // TODO - this can probably be validated at the CLI level
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
                    .flush(remote_cli_args.flush)
                    .throttle_time(remote_cli_args.throttle_time)
                    .throttle_range(remote_cli_args.throttle_range)
                    .get_args(remote_cli_args.get_args.clone())
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
                    .flush(remote_cli_args.flush)
                    .throttle_time(remote_cli_args.throttle_time)
                    .throttle_range(remote_cli_args.throttle_range)
                    .get_args(remote_cli_args.get_args.clone())
                    .build()
                    .unwrap(),
            )
        }
        (None, None) => None,
    };
    match (
        remote_cli_args.created_after.clone(),
        remote_cli_args.created_before.clone(),
    ) {
        (Some(created_after), Some(created_before)) => {
            if let Some(body_args) = &body_args {
                return Ok(Some(
                    ListBodyArgs::builder()
                        .page(body_args.page.unwrap())
                        .max_pages(body_args.max_pages.unwrap())
                        .created_after(Some(created_after.to_string()))
                        .created_before(Some(created_before.to_string()))
                        .sort_mode(remote_cli_args.sort.clone())
                        .flush(remote_cli_args.flush)
                        .throttle_time(remote_cli_args.throttle_time)
                        .throttle_range(remote_cli_args.throttle_range)
                        .get_args(remote_cli_args.get_args.clone())
                        .build()
                        .unwrap(),
                ));
            }
            return Ok(Some(
                ListBodyArgs::builder()
                    .created_after(Some(created_after.to_string()))
                    .created_before(Some(created_before.to_string()))
                    .sort_mode(remote_cli_args.sort.clone())
                    .flush(remote_cli_args.flush)
                    .throttle_time(remote_cli_args.throttle_time)
                    .throttle_range(remote_cli_args.throttle_range)
                    .get_args(remote_cli_args.get_args.clone())
                    .build()
                    .unwrap(),
            ));
        }
        (Some(created_after), None) => {
            if let Some(body_args) = &body_args {
                return Ok(Some(
                    ListBodyArgs::builder()
                        .page(body_args.page.unwrap())
                        .max_pages(body_args.max_pages.unwrap())
                        .created_after(Some(created_after.to_string()))
                        .sort_mode(remote_cli_args.sort.clone())
                        .flush(remote_cli_args.flush)
                        .throttle_time(remote_cli_args.throttle_time)
                        .throttle_range(remote_cli_args.throttle_range)
                        .get_args(remote_cli_args.get_args.clone())
                        .build()
                        .unwrap(),
                ));
            }
            return Ok(Some(
                ListBodyArgs::builder()
                    .created_after(Some(created_after.to_string()))
                    .sort_mode(remote_cli_args.sort.clone())
                    .flush(remote_cli_args.flush)
                    .throttle_time(remote_cli_args.throttle_time)
                    .throttle_range(remote_cli_args.throttle_range)
                    .get_args(remote_cli_args.get_args.clone())
                    .build()
                    .unwrap(),
            ));
        }
        (None, Some(created_before)) => {
            if let Some(body_args) = &body_args {
                return Ok(Some(
                    ListBodyArgs::builder()
                        .page(body_args.page.unwrap())
                        .max_pages(body_args.max_pages.unwrap())
                        .created_before(Some(created_before.to_string()))
                        .sort_mode(remote_cli_args.sort.clone())
                        .flush(remote_cli_args.flush)
                        .throttle_time(remote_cli_args.throttle_time)
                        .throttle_range(remote_cli_args.throttle_range)
                        .get_args(remote_cli_args.get_args.clone())
                        .build()
                        .unwrap(),
                ));
            }
            return Ok(Some(
                ListBodyArgs::builder()
                    .created_before(Some(created_before.to_string()))
                    .sort_mode(remote_cli_args.sort.clone())
                    .flush(remote_cli_args.flush)
                    .throttle_time(remote_cli_args.throttle_time)
                    .throttle_range(remote_cli_args.throttle_range)
                    .get_args(remote_cli_args.get_args.clone())
                    .build()
                    .unwrap(),
            ));
        }
        (None, None) => {
            if let Some(body_args) = &body_args {
                return Ok(Some(
                    ListBodyArgs::builder()
                        .page(body_args.page.unwrap())
                        .max_pages(body_args.max_pages.unwrap())
                        .sort_mode(remote_cli_args.sort.clone())
                        .flush(remote_cli_args.flush)
                        .throttle_time(remote_cli_args.throttle_time)
                        .throttle_range(remote_cli_args.throttle_range)
                        .get_args(remote_cli_args.get_args.clone())
                        .build()
                        .unwrap(),
                ));
            }
            return Ok(Some(
                ListBodyArgs::builder()
                    .sort_mode(remote_cli_args.sort.clone())
                    .flush(remote_cli_args.flush)
                    .throttle_time(remote_cli_args.throttle_time)
                    .throttle_range(remote_cli_args.throttle_range)
                    .get_args(remote_cli_args.get_args.clone())
                    .build()
                    .unwrap(),
            ));
        }
    }
}

pub struct URLQueryParamBuilder {
    url: String,
}

impl URLQueryParamBuilder {
    pub fn new(url: &str) -> Self {
        URLQueryParamBuilder {
            url: url.to_string(),
        }
    }

    pub fn add_param(&mut self, key: &str, value: &str) -> &mut Self {
        if self.url.contains('?') {
            self.url.push_str(&format!("&{}={}", key, value));
        } else {
            self.url.push_str(&format!("?{}={}", key, value));
        }
        self
    }

    pub fn build(&self) -> String {
        self.url.clone()
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum ListSortMode {
    #[default]
    Asc,
    Desc,
}

macro_rules! get {
    ($func_name:ident, $trait_name:ident) => {
        paste::paste! {
            pub fn $func_name(
                domain: String,
                path: String,
                config: Arc<Config>,
                cache_args: Option<&CacheCliArgs>,
            ) -> Result<Arc<dyn $trait_name + Send + Sync + 'static>> {
                let refresh_cache = cache_args.map_or(false, |args| args.refresh);
                let no_cache = cache_args.map_or(false, |args| args.no_cache);

                if no_cache {
                    let runner = Arc::new(http::Client::new(NoCache, config.clone(), refresh_cache));
                    [<create_remote_ $func_name>](domain, path, config, runner)
                } else {
                    let file_cache = FileCache::new(config.clone());
                    let runner = Arc::new(http::Client::new(file_cache, config.clone(), refresh_cache));
                    [<create_remote_ $func_name>](domain, path, config, runner)
                }
            }

            fn [<create_remote_ $func_name>]<R>(
                domain: String,
                path: String,
                config: Arc<Config>,
                runner: Arc<R>,
            ) -> Result<Arc<dyn $trait_name + Send + Sync + 'static>>
            where
                R: HttpRunner<Response = Response> + Send + Sync + 'static,
            {
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
        }
    };
}

get!(get_mr, MergeRequest);
get!(get_cicd, Cicd);
get!(get_project, RemoteProject);
get!(get_registry, ContainerRegistry);
get!(get_deploy, Deploy);
get!(get_deploy_asset, DeployAsset);
get!(get_auth_user, UserInfo);
get!(get_cicd_runner, CicdRunner);
get!(get_comment_mr, CommentMergeRequest);
get!(get_trending, TrendingProjectURL);
get!(get_gist, CodeGist);
get!(get_cicd_job, CicdJob);

pub fn extract_domain_path(repo_cli: &str) -> (String, String) {
    let parts: Vec<&str> = repo_cli.split('/').collect();
    let domain = parts[0].to_string();
    let path = parts[1..].join("/");
    (domain, path)
}

/// Given a CLI command, the command can work as long as user is in a cd
/// repository, user passes --domain flag (DomainArgs) or --repo flag
/// (RepoArgs). Some CLI commands might work with one variant, with both, with
/// all or might have no requirement at all.
pub enum CliDomainRequirements {
    CdInLocalRepo,
    DomainArgs,
    RepoArgs,
}

impl CliDomainRequirements {
    pub fn check<R: TaskRunner<Response = Response>>(
        &self,
        cli_args: &cli::CliArgs,
        runner: &R,
    ) -> Result<(String, String)> {
        match self {
            CliDomainRequirements::CdInLocalRepo => match git::remote_url(runner) {
                Ok(CmdInfo::RemoteUrl { domain, path }) => Ok((domain, path)),
                Err(err) => Err(GRError::GitRemoteUrlNotFound(format!("{}", err)).into()),
                _ => Err(GRError::ApplicationError(
                    "Could not get remote url during startup. \
                        main::get_config_domain_path - Please open a bug to \
                        https://github.com/jordilin/gitar"
                        .to_string(),
                )
                .into()),
            },
            CliDomainRequirements::DomainArgs => {
                if cli_args.domain.is_some() {
                    Ok((
                        cli_args.domain.as_ref().unwrap().to_string(),
                        "".to_string(),
                    ))
                } else {
                    Err(GRError::DomainExpected("Missing domain information".to_string()).into())
                }
            }
            CliDomainRequirements::RepoArgs => {
                if cli_args.repo.is_some() {
                    Ok(extract_domain_path(cli_args.repo.as_ref().unwrap()))
                } else {
                    Err(GRError::RepoExpected("Missing repository information".to_string()).into())
                }
            }
        }
    }
}

impl Display for CliDomainRequirements {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            CliDomainRequirements::CdInLocalRepo => write!(f, "cd to a git repository"),
            CliDomainRequirements::DomainArgs => write!(f, "provide --domain option"),
            CliDomainRequirements::RepoArgs => write!(f, "provide --repo option"),
        }
    }
}

pub fn get_domain_path<R: TaskRunner<Response = Response>>(
    cli_args: &cli::CliArgs,
    requirements: &[CliDomainRequirements],
    runner: &R,
) -> Result<(String, String)> {
    let mut errors = Vec::new();
    for requirement in requirements {
        match requirement.check(cli_args, runner) {
            Ok((d, p)) => return Ok((d, p)),
            Err(err) => {
                errors.push(err);
            }
        }
    }
    let trace = errors
        .iter()
        .map(|e| format!("{}", e))
        .collect::<Vec<String>>()
        .join("\n");

    let expectations_missed_trace = requirements
        .iter()
        .map(|r| format!("{}", r))
        .collect::<Vec<String>>()
        .join(" OR ");

    Err(GRError::PreconditionNotMet(format!(
        "\n\nMissed requirements: {}\n\n Errors:\n\n {}",
        expectations_missed_trace, trace
    ))
    .into())
}

pub fn read_config(config_file: &Path, domain: &str) -> Result<Arc<Config>> {
    let f = File::open(config_file)?;
    let config = Config::new(f, domain)?;
    Ok(Arc::new(config))
}

#[cfg(test)]
mod test {
    use cli::CliArgs;

    use crate::test::utils::MockRunner;

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

    #[test]
    fn test_add_created_before_with_page_number() {
        let page_number = Some(1);
        let created_before = "2021-01-01T00:00:00Z";
        let args = ListRemoteCliArgs::builder()
            .page_number(page_number)
            .created_before(Some(created_before.to_string()))
            .build()
            .unwrap();
        let args = validate_from_to_page(&args).unwrap().unwrap();
        assert_eq!(args.page.unwrap(), 1);
        assert_eq!(args.max_pages.unwrap(), 1);
        assert_eq!(args.created_before.unwrap(), created_before);
    }

    #[test]
    fn test_add_created_before_with_from_to_page() {
        let from_page = Some(1);
        let to_page = Some(3);
        let created_before = "2021-01-01T00:00:00Z";
        let args = ListRemoteCliArgs::builder()
            .from_page(from_page)
            .to_page(to_page)
            .created_before(Some(created_before.to_string()))
            .sort(ListSortMode::Desc)
            .build()
            .unwrap();
        let args = validate_from_to_page(&args).unwrap().unwrap();
        assert_eq!(args.page.unwrap(), 1);
        assert_eq!(args.max_pages.unwrap(), 3);
        assert_eq!(args.created_before.unwrap(), created_before);
        assert_eq!(args.sort_mode, ListSortMode::Desc);
    }

    #[test]
    fn test_add_crated_before_with_no_created_after_option_and_no_page_number() {
        let created_before = "2021-01-01T00:00:00Z";
        let args = ListRemoteCliArgs::builder()
            .created_before(Some(created_before.to_string()))
            .sort(ListSortMode::Desc)
            .build()
            .unwrap();
        let args = validate_from_to_page(&args).unwrap().unwrap();
        assert_eq!(args.created_before.unwrap(), created_before);
        assert_eq!(args.sort_mode, ListSortMode::Desc);
    }

    #[test]
    fn test_adds_created_after_and_created_before_with_from_to_page() {
        let from_page = Some(1);
        let to_page = Some(3);
        let created_after = "2021-01-01T00:00:00Z";
        let created_before = "2021-01-02T00:00:00Z";
        let args = ListRemoteCliArgs::builder()
            .from_page(from_page)
            .to_page(to_page)
            .created_after(Some(created_after.to_string()))
            .created_before(Some(created_before.to_string()))
            .sort(ListSortMode::Desc)
            .build()
            .unwrap();
        let args = validate_from_to_page(&args).unwrap().unwrap();
        assert_eq!(args.page.unwrap(), 1);
        assert_eq!(args.max_pages.unwrap(), 3);
        assert_eq!(args.created_after.unwrap(), created_after);
        assert_eq!(args.created_before.unwrap(), created_before);
        assert_eq!(args.sort_mode, ListSortMode::Desc);
    }

    #[test]
    fn test_add_created_after_and_before_no_from_to_page_options() {
        let created_after = "2021-01-01T00:00:00Z";
        let created_before = "2021-01-02T00:00:00Z";
        let args = ListRemoteCliArgs::builder()
            .created_after(Some(created_after.to_string()))
            .created_before(Some(created_before.to_string()))
            .sort(ListSortMode::Desc)
            .build()
            .unwrap();
        let args = validate_from_to_page(&args).unwrap().unwrap();
        assert_eq!(args.created_after.unwrap(), created_after);
        assert_eq!(args.created_before.unwrap(), created_before);
        assert_eq!(args.sort_mode, ListSortMode::Desc);
    }

    #[test]
    fn test_if_only_to_page_provided_max_pages_is_to_page() {
        let to_page = Some(3);
        let args = ListRemoteCliArgs::builder()
            .to_page(to_page)
            .build()
            .unwrap();
        let args = validate_from_to_page(&args).unwrap().unwrap();
        assert_eq!(args.page, Some(1));
        assert_eq!(args.max_pages, Some(3));
    }

    #[test]
    fn test_if_only_to_page_provided_and_negative_number_is_error() {
        let to_page = Some(-3);
        let args = ListRemoteCliArgs::builder()
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
    fn test_if_sort_provided_use_it() {
        let args = ListRemoteCliArgs::builder()
            .sort(ListSortMode::Desc)
            .build()
            .unwrap();
        let args = validate_from_to_page(&args).unwrap().unwrap();
        assert_eq!(args.sort_mode, ListSortMode::Desc);
    }

    #[test]
    fn test_if_flush_option_provided_use_it() {
        let args = ListRemoteCliArgs::builder().flush(true).build().unwrap();
        let args = validate_from_to_page(&args).unwrap().unwrap();
        assert!(args.flush);
    }

    #[test]
    fn test_query_param_builder_no_params() {
        let url = "https://example.com";
        let url = URLQueryParamBuilder::new(url).build();
        assert_eq!(url, "https://example.com");
    }

    #[test]
    fn test_query_param_builder_with_params() {
        let url = "https://example.com";
        let url = URLQueryParamBuilder::new(url)
            .add_param("key", "value")
            .add_param("key2", "value2")
            .build();
        assert_eq!(url, "https://example.com?key=value&key2=value2");
    }

    #[test]
    fn test_retrieve_domain_path_from_repo_cli_flag() {
        let repo_cli = "github.com/jordilin/gitar";
        let (domain, path) = extract_domain_path(repo_cli);
        assert_eq!("github.com", domain);
        assert_eq!("jordilin/gitar", path);
    }

    #[test]
    fn test_cli_requires_cd_local_repo_run_git_remote() {
        let cli_args = CliArgs::new(0, None, None, None);
        let response = Response::builder()
            .body("git@github.com:jordilin/gitar.git".to_string())
            .build()
            .unwrap();
        let runner = MockRunner::new(vec![response]);
        let requirements = vec![CliDomainRequirements::CdInLocalRepo];
        let (domain, path) = get_domain_path(&cli_args, &requirements, &runner).unwrap();
        assert_eq!("github.com", domain);
        assert_eq!("jordilin/gitar", path);
    }

    #[test]
    fn test_cli_requires_cd_local_repo_run_git_remote_error() {
        let cli_args = CliArgs::new(0, None, None, None);
        let response = Response::builder().body("".to_string()).build().unwrap();
        let runner = MockRunner::new(vec![response]);
        let requirements = vec![CliDomainRequirements::CdInLocalRepo];
        let result = get_domain_path(&cli_args, &requirements, &runner);
        match result {
            Err(err) => match err.downcast_ref::<error::GRError>() {
                Some(error::GRError::PreconditionNotMet(_)) => (),
                _ => panic!("Expected error::GRError::GitRemoteUrlNotFound"),
            },
            _ => panic!("Expected error"),
        }
    }

    #[test]
    fn test_cli_requires_repo_args_or_cd_repo_fails_on_cd_repo() {
        let cli_args = CliArgs::new(0, Some("github.com/jordilin/gitar".to_string()), None, None);
        let requirements = vec![
            CliDomainRequirements::CdInLocalRepo,
            CliDomainRequirements::RepoArgs,
        ];
        let response = Response::builder().body("".to_string()).build().unwrap();
        let (domain, path) =
            get_domain_path(&cli_args, &requirements, &MockRunner::new(vec![response])).unwrap();
        assert_eq!("github.com", domain);
        assert_eq!("jordilin/gitar", path);
    }

    #[test]
    fn test_cli_requires_domain_args_or_cd_repo_fails_on_cd_repo() {
        let cli_args = CliArgs::new(0, None, Some("github.com".to_string()), None);
        let requirements = vec![
            CliDomainRequirements::CdInLocalRepo,
            CliDomainRequirements::DomainArgs,
        ];
        let response = Response::builder().body("".to_string()).build().unwrap();
        let (domain, path) =
            get_domain_path(&cli_args, &requirements, &MockRunner::new(vec![response])).unwrap();
        assert_eq!("github.com", domain);
        assert_eq!("", path);
    }
}
