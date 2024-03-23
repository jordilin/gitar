use clap::Parser;

use crate::{cmds::project::ProjectMetadataGetCliArgs, display::Format};

use super::common::GetArgs;

#[derive(Parser)]
pub struct ProjectCommand {
    #[clap(subcommand)]
    subcommand: ProjectSubcommand,
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
    #[clap(flatten)]
    pub get_args: GetArgs,
}

impl From<ProjectCommand> for ProjectOptions {
    fn from(options: ProjectCommand) -> Self {
        match options.subcommand {
            ProjectSubcommand::Info(options) => ProjectOptions {
                operation: options.into(),
            },
        }
    }
}

impl From<ProjectInfo> for ProjectOperation {
    fn from(options: ProjectInfo) -> Self {
        ProjectOperation::Info(ProjectMetadataGetCliArgs {
            id: options.id,
            get_args: options.get_args.into(),
        })
    }
}

pub enum ProjectOperation {
    Info(ProjectMetadataGetCliArgs),
}

pub struct ProjectOptions {
    pub operation: ProjectOperation,
}
