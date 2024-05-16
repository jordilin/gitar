use std::fmt::{self, Display, Formatter};

use clap::{Parser, ValueEnum};

use crate::{
    display::Format,
    remote::{GetRemoteCliArgs, ListRemoteCliArgs, ListSortMode},
    time::Milliseconds,
};

#[derive(Clone, Parser)]
#[clap(next_help_heading = "List options")]
pub struct ListArgs {
    /// List the given page number
    #[clap(long)]
    page: Option<i64>,
    /// From page
    #[clap(long)]
    pub from_page: Option<i64>,
    /// To page
    #[clap(long)]
    pub to_page: Option<i64>,
    /// How many pages are available
    #[clap(long)]
    num_pages: bool,
    /// Created after date (ISO 8601 YYYY-MM-DDTHH:MM:SSZ)
    #[clap(long)]
    created_after: Option<String>,
    /// Created before date (ISO 8601 YYYY-MM-DDTHH:MM:SSZ)
    #[clap(long)]
    created_before: Option<String>,
    /// Flush results to STDOUT as they are received. No sorting and no date
    /// filtering is applied
    #[clap(long, visible_alias = "flush")]
    pub stream: bool,
    /// Throttle the requests to the server. Time to wait in milliseconds
    /// between each HTTP request.
    #[clap(long, value_name = "MILLISECONDS")]
    pub throttle: Option<u64>,
    #[clap(long, default_value_t=SortModeCli::Asc)]
    sort: SortModeCli,
    #[clap(flatten)]
    pub get_args: GetArgs,
}

#[derive(Clone, Parser)]
pub struct GetArgs {
    #[clap(flatten)]
    pub format_args: FormatArgs,
    /// Refresh the cache
    #[clap(long, short, help_heading = "Cache options")]
    pub refresh: bool,
    #[clap(flatten)]
    pub retry_args: RetryArgs,
}

#[derive(Clone, Parser)]
#[clap(next_help_heading = "Formatting options")]
pub struct FormatArgs {
    /// Do not print headers
    #[clap(long)]
    pub no_headers: bool,
    /// Output format
    #[clap(long, default_value_t=FormatCli::Pipe)]
    pub format: FormatCli,
    /// Display additional fields
    #[clap(visible_short_alias = 'o', long)]
    pub more_output: bool,
}

#[derive(Clone, Parser)]
#[clap(next_help_heading = "Retry options")]
pub struct RetryArgs {
    /// Retries request on error. Backs off exponentially if enabled
    #[clap(long)]
    pub backoff: bool,
    /// Number of retries
    #[clap(long, default_value = "0", requires = "backoff")]
    pub max_retries: u32,
    /// Additional delay in seconds before retrying the request when backoff is
    /// enabled
    #[clap(long, default_value = "60", requires = "backoff")]
    pub retry_after: u64,
}

#[derive(ValueEnum, Clone, Debug)]
pub enum FormatCli {
    Csv,
    Json,
    Pipe,
}

impl Display for FormatCli {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            FormatCli::Csv => write!(f, "csv"),
            FormatCli::Pipe => write!(f, "pipe"),
            FormatCli::Json => write!(f, "json"),
        }
    }
}

#[derive(ValueEnum, Clone, Debug)]
enum SortModeCli {
    Asc,
    Desc,
}

impl Display for SortModeCli {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            SortModeCli::Asc => write!(f, "asc"),
            SortModeCli::Desc => write!(f, "desc"),
        }
    }
}

impl From<ListArgs> for ListRemoteCliArgs {
    fn from(args: ListArgs) -> Self {
        ListRemoteCliArgs::builder()
            .from_page(args.from_page)
            .to_page(args.to_page)
            .page_number(args.page)
            .num_pages(args.num_pages)
            .created_after(args.created_after)
            .created_before(args.created_before)
            .sort(args.sort.into())
            .get_args(args.get_args.into())
            .flush(args.stream)
            .throttle_time(args.throttle.map(Milliseconds::from))
            .build()
            .unwrap()
    }
}

impl From<GetArgs> for GetRemoteCliArgs {
    fn from(args: GetArgs) -> Self {
        GetRemoteCliArgs::builder()
            .no_headers(args.format_args.no_headers)
            .format(args.format_args.format.into())
            .display_optional(args.format_args.more_output)
            .refresh_cache(args.refresh)
            .backoff_max_retries(args.retry_args.max_retries)
            .backoff_retry_after(args.retry_args.retry_after)
            .build()
            .unwrap()
    }
}

impl From<FormatCli> for Format {
    fn from(format: FormatCli) -> Self {
        match format {
            FormatCli::Csv => Format::CSV,
            FormatCli::Json => Format::JSON,
            FormatCli::Pipe => Format::PIPE,
        }
    }
}

impl From<SortModeCli> for ListSortMode {
    fn from(sort: SortModeCli) -> Self {
        match sort {
            SortModeCli::Asc => ListSortMode::Asc,
            SortModeCli::Desc => ListSortMode::Desc,
        }
    }
}

pub fn validate_project_repo_path(path: &str) -> Result<String, String> {
    let (fields, empty_fields) = fields(path);
    if fields.count() == 2 && empty_fields == 0 {
        Ok(path.to_string())
    } else {
        Err("Path must be in the format `OWNER/PROJECT_NAME`".to_string())
    }
}

pub fn validate_domain_project_repo_path(path: &str) -> Result<String, String> {
    let (fields, empty_fields) = fields(path);
    if fields.count() == 3 && empty_fields == 0 {
        Ok(path.to_string())
    } else {
        Err("Path must be in the format `DOMAIN/OWNER/PROJECT_NAME`".to_string())
    }
}

fn fields(path: &str) -> (std::str::Split<char>, usize) {
    let fields = path.split('/');
    let empty_fields = fields.clone().filter(|f| f.is_empty()).count();
    (fields, empty_fields)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_validate_project_repo_path() {
        assert!(validate_project_repo_path("owner/project").is_ok());
        assert!(validate_project_repo_path("owner/project/extra").is_err());
        assert!(validate_project_repo_path("owner").is_err());
        assert!(validate_project_repo_path("owner/project/extra/extra").is_err());
        assert!(validate_project_repo_path("owner/").is_err());
    }

    #[test]
    fn test_validate_domain_project_repo_path() {
        assert!(validate_domain_project_repo_path("github.com/jordilin/gitar").is_ok());
        assert!(validate_domain_project_repo_path("github.com/jordilin/").is_err());
        assert!(validate_domain_project_repo_path("github.com///").is_err());
        assert!(validate_domain_project_repo_path("github.com/jordilin/project/extra").is_err());
        assert!(validate_domain_project_repo_path("github.com/jordilin").is_err());
        assert!(
            validate_domain_project_repo_path("github.com/jordilin/project/extra/extra").is_err()
        );
    }
}
