pub mod amps;
pub mod browse;
pub mod cache;
pub mod cicd;
pub mod common;
pub mod docker;
pub mod init;
pub mod merge_request;
pub mod my;
pub mod project;
pub mod release;
pub mod star;
pub mod trending;
pub mod user;

use self::browse::BrowseCommand;
use self::browse::BrowseOptions;
use self::cicd::{PipelineCommand, PipelineOptions};
use self::common::validate_domain_project_repo_path;
use self::docker::{DockerCommand, DockerOptions};
use self::init::{InitCommand, InitCommandOptions};
use self::my::MyCommand;
use self::my::MyOptions;
use self::project::{ProjectCommand, ProjectOptions};
use self::release::{ReleaseCommand, ReleaseOptions};
use self::trending::TrendingCommand;
use self::trending::TrendingOptions;
use amps::AmpsCommand;
use amps::AmpsOptions;
use cache::CacheCommand;
use cache::CacheOptions;
use clap::ArgAction;
use merge_request::{MergeRequestCommand, MergeRequestOptions};
use user::UserCommand;
use user::UserOptions;

use std::option::Option;

use clap::builder::{styling::AnsiColor, Styles};
use clap::Parser;

const CLI_STYLE: Styles = Styles::styled()
    .header(AnsiColor::Red.on_default().bold())
    .literal(AnsiColor::Blue.on_default().bold())
    .placeholder(AnsiColor::Green.on_default())
    .usage(AnsiColor::Red.on_default().bold());

#[derive(Parser)]
#[command(about = "A Github/Gitlab CLI tool", styles = CLI_STYLE, version)]
#[clap(next_help_heading = "Global options")]
struct Args {
    #[clap(subcommand)]
    pub command: Command,
    /// Verbose mode. Enable gitar's logging. -v for INFO, -vv for DEBUG
    #[clap(long, short, global = true, action = ArgAction::Count)]
    verbose: u8,
    /// Bypass local .git/config. Use repo instead. Ex: github.com/jordilin/gitar
    #[clap(
        long,
        global = true,
        value_name = "DOMAIN/OWNER/PROJECT_NAME",
        value_parser = validate_domain_project_repo_path
    )]
    pub repo: Option<String>,
    /// Bypass local .git/config. Use domain. Ex. for my subcommands
    #[clap(long, global = true, value_name = "DOMAIN")]
    pub domain: Option<String>,
    /// Full path to the config location. Default is $HOME/.config/gitar/api
    #[clap(long, global = true, value_name = "PATH")]
    pub config: Option<String>,
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
    #[clap(name = "tr", about = "Trending repositories. Github.com only.")]
    Trending(TrendingCommand),
    #[clap(name = "us", about = "User operations")]
    User(UserCommand),
    /// Interactively execute gitar amplifier commands using gitar. gr-in-gr
    #[clap(name = "amps")]
    Amps(AmpsCommand),
    #[clap(name = "init", about = "Initialize the config file")]
    Init(InitCommand),
    #[clap(name = "cache", about = "Local cache operations")]
    Cache(CacheCommand),
    #[clap(
        name = "manual",
        about = "Open the user manual in the browser",
        visible_alias = "man"
    )]
    Manual,
}

// Parse cli and return CliOptions
pub fn parse_cli() -> OptionArgs {
    let args = Args::parse();
    let options = match args.command {
        Command::MergeRequest(sub_matches) => Some(CliOptions::MergeRequest(sub_matches.into())),
        Command::Browse(sub_matches) => Some(CliOptions::Browse(sub_matches.into())),
        Command::Pipeline(sub_matches) => Some(CliOptions::Pipeline(sub_matches.into())),
        Command::Project(sub_matches) => Some(CliOptions::Project(sub_matches.into())),
        Command::Init(sub_matches) => Some(CliOptions::Init(sub_matches.into())),
        Command::Docker(sub_matches) => Some(CliOptions::Docker(sub_matches.into())),
        Command::Release(sub_matches) => Some(CliOptions::Release(sub_matches.into())),
        Command::My(sub_matches) => Some(CliOptions::My(sub_matches.into())),
        Command::Trending(sub_matches) => Some(CliOptions::Trending(sub_matches.into())),
        Command::Cache(sub_matches) => Some(CliOptions::Cache(sub_matches.into())),
        Command::Manual => Some(CliOptions::Manual),
        Command::Amps(sub_matches) => Some(CliOptions::Amps(sub_matches.into())),
        Command::User(sub_matches) => Some(CliOptions::User(sub_matches.into())),
    };
    OptionArgs::new(
        options,
        CliArgs::new(args.verbose, args.repo, args.domain, args.config),
    )
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
    Trending(TrendingOptions),
    Cache(CacheOptions),
    Manual,
    Amps(AmpsOptions),
    User(UserOptions),
}

#[derive(Clone)]
pub struct CliArgs {
    pub verbose: u8,
    pub repo: Option<String>,
    pub domain: Option<String>,
    pub config: Option<String>,
}

impl CliArgs {
    pub fn new(
        verbose: u8,
        repo: Option<String>,
        domain: Option<String>,
        config: Option<String>,
    ) -> Self {
        CliArgs {
            verbose,
            repo,
            domain,
            config,
        }
    }
}

pub struct OptionArgs {
    pub cli_options: Option<CliOptions>,
    pub cli_args: CliArgs,
}

impl OptionArgs {
    pub fn new(cli_options: Option<CliOptions>, cli_args: CliArgs) -> Self {
        OptionArgs {
            cli_options,
            cli_args,
        }
    }
}
