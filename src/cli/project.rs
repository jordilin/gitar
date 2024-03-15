use clap::Parser;

use crate::display::Format;

use super::common::FormatCli;

#[derive(Parser)]
pub struct ProjectCommand {
    #[clap(subcommand)]
    subcommand: ProjectSubcommand,
    /// Refresh the cache
    #[clap(long, short)]
    pub refresh: bool,
}

#[derive(Parser)]
enum ProjectSubcommand {
    #[clap(about = "Gather project information metadata")]
    Info(ProjectInfo),
}

#[derive(Parser)]
struct ProjectInfo {
    /// ID of the project
    #[clap(long)]
    pub id: Option<i64>,
    /// Output format. pipe " | " or csv ","
    #[clap(long, default_value_t=FormatCli::Pipe)]
    format: FormatCli,
}

impl From<ProjectCommand> for ProjectOptions {
    fn from(options: ProjectCommand) -> Self {
        match options.subcommand {
            ProjectSubcommand::Info(options_info) => ProjectOptions {
                operation: ProjectOperation::Info {
                    id: options_info.id,
                },
                refresh_cache: options.refresh,
                format: options_info.format.into(),
            },
        }
    }
}

#[derive(Debug)]
pub enum ProjectOperation {
    Info { id: Option<i64> },
}

#[derive(Debug)]
pub struct ProjectOptions {
    pub operation: ProjectOperation,
    pub refresh_cache: bool,
    pub format: Format,
}
