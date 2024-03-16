pub mod browse;
pub mod cicd;
pub mod common;
pub mod docker;
pub mod init;
pub mod merge_request;
pub mod my;
pub mod project;
pub mod release;

use self::browse::BrowseCommand;
use self::browse::BrowseOptions;
use self::cicd::{PipelineCommand, PipelineOptions};
use self::docker::{DockerCommand, DockerOptions};
use self::init::{InitCommand, InitCommandOptions};
use self::my::MyCommand;
use self::my::MyOptions;
use self::project::{ProjectCommand, ProjectOptions};
use self::release::{ReleaseCommand, ReleaseOptions};
use merge_request::{MergeRequestCommand, MergeRequestOptions};

use std::option::Option;

use clap::Parser;

#[derive(Parser)]
#[command(about = "A Github/Gitlab CLI tool")]
struct Args {
    #[clap(subcommand)]
    pub command: Command,
}

#[derive(Parser)]
enum Command {
    #[clap(name = "mr", about = "Merge request operations")]
    MergeRequest(MergeRequestCommand),
    #[clap(name = "br", about = "Open the remote using your browser")]
    Browse(BrowseCommand),
    #[clap(name = "pp", about = "CI/CD Pipeline operations")]
    Pipeline(PipelineCommand),
    #[clap(name = "pj", about = "Gather project information metadata")]
    Project(ProjectCommand),
    #[clap(
        name = "dk",
        about = "Handles docker images in Gitlab/Github registries"
    )]
    Docker(DockerCommand),
    #[clap(name = "rl", about = "Release operations")]
    Release(ReleaseCommand),
    #[clap(
        name = "my",
        about = "Your user information, such as assigned merge requests, etc..."
    )]
    My(MyCommand),
    #[clap(name = "init", about = "Initialize the config file")]
    Init(InitCommand),
}

// Parse cli and return CliOptions
pub fn parse_cli() -> Option<CliOptions> {
    let args = Args::parse();
    match args.command {
        Command::MergeRequest(sub_matches) => Some(CliOptions::MergeRequest(sub_matches.into())),
        Command::Browse(sub_matches) => Some(CliOptions::Browse(sub_matches.into())),
        Command::Pipeline(sub_matches) => Some(CliOptions::Pipeline(sub_matches.into())),
        Command::Project(sub_matches) => Some(CliOptions::Project(sub_matches.into())),
        Command::Init(sub_matches) => Some(CliOptions::Init(sub_matches.into())),
        Command::Docker(sub_matches) => Some(CliOptions::Docker(sub_matches.into())),
        Command::Release(sub_matches) => Some(CliOptions::Release(sub_matches.into())),
        Command::My(sub_matches) => Some(CliOptions::My(sub_matches.into())),
    }
}

pub enum CliOptions {
    MergeRequest(MergeRequestOptions),
    Browse(BrowseOptions),
    Pipeline(PipelineOptions),
    Project(ProjectOptions),
    Init(InitCommandOptions),
    Docker(DockerOptions),
    Release(ReleaseOptions),
    My(MyOptions),
}
