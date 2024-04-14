use clap::Parser;

use super::project::ListProject;

#[derive(Parser)]
pub struct StarsCommand {
    #[clap(subcommand)]
    subcommand: StarsSubCommand,
}

#[derive(Parser)]
enum StarsSubCommand {
    #[clap(about = "List starred projects")]
    List(ListProject),
}
