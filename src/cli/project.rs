use clap::Parser;

use crate::cmds::project::ProjectMetadataGetCliArgs;

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
            ProjectSubcommand::Info(options) => options.into(),
        }
    }
}

impl From<ProjectInfo> for ProjectOptions {
    fn from(options: ProjectInfo) -> Self {
        ProjectOptions::Info(
            ProjectMetadataGetCliArgs::builder()
                .id(options.id)
                .get_args(options.get_args.into())
                .build()
                .unwrap(),
        )
    }
}

pub enum ProjectOptions {
    Info(ProjectMetadataGetCliArgs),
}

#[cfg(test)]
mod test {
    use crate::cli::{Args, Command};

    use super::*;

    #[test]
    fn test_project_cli_info() {
        let args = Args::parse_from(vec!["gr", "pj", "info", "--id", "1"]);
        let project_info = match args.command {
            Command::Project(ProjectCommand {
                subcommand: ProjectSubcommand::Info(options),
            }) => {
                assert_eq!(options.id, Some(1));
                options
            }
            _ => panic!("Expected ProjectCommand::Info"),
        };
        let options: ProjectOptions = project_info.into();
        match options {
            ProjectOptions::Info(options) => {
                assert_eq!(options.id, Some(1));
            }
        }
    }
}
