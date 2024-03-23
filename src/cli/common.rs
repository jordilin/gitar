use std::fmt::{self, Display, Formatter};

use clap::{Parser, ValueEnum};

use crate::{
    display::Format,
    remote::{ListRemoteCliArgs, ListSortMode},
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

pub fn gen_list_args(list_args: ListArgs) -> ListRemoteCliArgs {
    let list_args = ListRemoteCliArgs::builder()
        .from_page(list_args.from_page)
        .to_page(list_args.to_page)
        .page_number(list_args.page)
        .num_pages(list_args.num_pages)
        .refresh_cache(list_args.get_args.refresh)
        .no_headers(list_args.get_args.no_headers)
        .created_after(list_args.created_after)
        .created_before(list_args.created_before)
        .sort(list_args.sort.into())
        .format(list_args.get_args.format.into())
        .build()
        .unwrap();
    list_args
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
