use std::fmt::{self, Display, Formatter};

use clap::{Parser, ValueEnum};

use crate::{
    display::Format,
    remote::{CacheCliArgs, GetRemoteCliArgs, ListRemoteCliArgs, ListSortMode},
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
    /// How many resources are available. Result is an approximation depending
    /// on total pages and default per_page query param. Total given as an
    /// interval (min, max)
    #[clap(long)]
    pub num_resources: bool,
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
    /// Throttle the requests to the server. Fixed time to wait in milliseconds
    /// between each HTTP request.
    #[clap(long, value_name = "MILLISECONDS", group = "throttle_arg")]
    pub throttle: Option<u64>,
    /// Throttle the requests using a random wait time between the given range.
    /// The MIN and MAX values are in milliseconds.
    #[arg(long, value_parser=parse_throttle_range, value_name = "MIN-MAX", group = "throttle_arg")]
    throttle_range: Option<(u64, u64)>,
    #[clap(long, default_value_t=SortModeCli::Asc)]
    sort: SortModeCli,
    #[clap(flatten)]
    pub get_args: GetArgs,
}

#[derive(Clone, Parser)]
pub struct GetArgs {
    #[clap(flatten)]
    pub format_args: FormatArgs,
    #[clap(flatten)]
    pub cache_args: CacheArgs,
    #[clap(flatten)]
    pub retry_args: RetryArgs,
}

#[derive(Clone, Parser)]
#[clap(next_help_heading = "Cache options")]
pub struct CacheArgs {
    /// Refresh the cache
    #[clap(long, short, group = "cache")]
    pub refresh: bool,
    /// Disable caching data on disk
    #[clap(long, group = "cache")]
    pub no_cache: bool,
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
    Toml,
}

impl Display for FormatCli {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            FormatCli::Csv => write!(f, "csv"),
            FormatCli::Pipe => write!(f, "pipe"),
            FormatCli::Json => write!(f, "json"),
            FormatCli::Toml => write!(f, "toml"),
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
            .num_resources(args.num_resources)
            .created_after(args.created_after)
            .created_before(args.created_before)
            .sort(args.sort.into())
            .get_args(args.get_args.into())
            .flush(args.stream)
            .throttle_time(args.throttle.map(Milliseconds::from))
            .throttle_range(
                args.throttle_range
                    .map(|(min, max)| (Milliseconds::from(min), Milliseconds::from(max))),
            )
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
            .cache_args(args.cache_args.into())
            .backoff_max_retries(args.retry_args.max_retries)
            .backoff_retry_after(args.retry_args.retry_after)
            .build()
            .unwrap()
    }
}

impl From<CacheArgs> for CacheCliArgs {
    fn from(args: CacheArgs) -> Self {
        CacheCliArgs::builder()
            .refresh(args.refresh)
            .no_cache(args.no_cache)
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
            FormatCli::Toml => Format::TOML,
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

fn fields(path: &str) -> (std::str::Split<'_, char>, usize) {
    let fields = path.split('/');
    let empty_fields = fields.clone().filter(|f| f.is_empty()).count();
    (fields, empty_fields)
}

fn parse_throttle_range(s: &str) -> Result<(u64, u64), String> {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 2 {
        return Err(String::from("Throttle range must be in the format min-max"));
    }
    let min = parts[0].parse::<u64>().map_err(|_| "Invalid MIN value")?;
    let max = parts[1].parse::<u64>().map_err(|_| "Invalid MAX value")?;
    if min >= max {
        return Err(String::from("MIN must be less than MAX"));
    }
    Ok((min, max))
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

    #[test]
    fn test_valid_throttle_range() {
        assert_eq!(parse_throttle_range("100-500"), Ok((100, 500)));
        assert_eq!(parse_throttle_range("0-1000"), Ok((0, 1000)));
        assert_eq!(parse_throttle_range("1-2"), Ok((1, 2)));
    }

    #[test]
    fn test_invalid_number_of_arguments() {
        assert!(parse_throttle_range("100").is_err());
        assert!(parse_throttle_range("100-200 300").is_err());
        assert!(parse_throttle_range("").is_err());
    }

    #[test]
    fn test_invalid_number_format() {
        assert!(parse_throttle_range("abc-500").is_err());
        assert!(parse_throttle_range("100-def").is_err());
        assert!(parse_throttle_range("100.5-500").is_err());
    }

    #[test]
    fn test_min_greater_than_or_equal_to_max() {
        assert!(parse_throttle_range("500-100").is_err());
        assert!(parse_throttle_range("100-100").is_err());
    }

    #[test]
    fn test_error_messages() {
        assert_eq!(
            parse_throttle_range("100"),
            Err("Throttle range must be in the format min-max".to_string())
        );
        assert_eq!(
            parse_throttle_range("abc-500"),
            Err("Invalid MIN value".to_string())
        );
        assert_eq!(
            parse_throttle_range("100-def"),
            Err("Invalid MAX value".to_string())
        );
        assert_eq!(
            parse_throttle_range("500-100"),
            Err("MIN must be less than MAX".to_string())
        );
    }
}
