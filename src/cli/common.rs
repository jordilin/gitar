use std::fmt::{self, Display, Formatter};

use clap::{Parser, ValueEnum};

use crate::{
    display::Format,
    remote::{GetRemoteCliArgs, ListRemoteCliArgs, ListSortMode},
};

#[derive(Clone, Parser)]
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
    #[clap(long, default_value_t=SortModeCli::Asc)]
    sort: SortModeCli,
    #[clap(flatten)]
    pub get_args: GetArgs,
}

#[derive(Clone, Parser)]
pub struct GetArgs {
    /// Do not print headers
    #[clap(long)]
    pub no_headers: bool,
    /// Output format
    #[clap(long, default_value_t=FormatCli::Pipe)]
    pub format: FormatCli,
    /// Display additional fields
    #[clap(visible_short_alias = 'o', long)]
    pub more_output: bool,
    /// Refresh the cache
    #[clap(long, short)]
    pub refresh: bool,
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
            .build()
            .unwrap()
    }
}

impl From<GetArgs> for GetRemoteCliArgs {
    fn from(args: GetArgs) -> Self {
        GetRemoteCliArgs {
            refresh_cache: args.refresh,
            no_headers: args.no_headers,
            format: args.format.into(),
            display_optional: args.more_output,
        }
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
