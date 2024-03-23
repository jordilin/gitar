use clap::Parser;

use crate::cmds::docker::{DockerImageCliArgs, DockerListCliArgs};

use super::common::{GetArgs, ListArgs};

#[derive(Parser)]
pub struct DockerCommand {
    #[clap(subcommand)]
    subcommand: DockerSubCommand,
}

#[derive(Parser)]
enum DockerSubCommand {
    #[clap(about = "List Docker images")]
    List(ListDockerImages),
    #[clap(about = "Get docker image metadata")]
    Image(DockerImageMetadata),
}

#[derive(Parser)]
struct DockerImageMetadata {
    /// Repository ID the image belongs to
    #[clap(long)]
    repo_id: i64,
    /// Tag name
    #[clap()]
    tag: String,
    #[clap(flatten)]
    get_args: GetArgs,
}

#[derive(Parser)]
struct ListDockerImages {
    /// List image repositories in this projects' registry
    #[clap(long, default_value = "false", group = "list")]
    repos: bool,
    /// List all image tags for a given repository id
    #[clap(long, default_value = "false", group = "list", requires = "repo_id")]
    tags: bool,
    /// Repository ID to pull image tags from
    #[clap(long)]
    repo_id: Option<i64>,
    #[command(flatten)]
    list_args: ListArgs,
}

impl From<DockerCommand> for DockerOptions {
    fn from(options: DockerCommand) -> Self {
        match options.subcommand {
            DockerSubCommand::List(options) => options.into(),
            DockerSubCommand::Image(options) => options.into(),
        }
    }
}

impl From<DockerImageMetadata> for DockerOptions {
    fn from(options: DockerImageMetadata) -> Self {
        DockerOptions::Get(
            DockerImageCliArgs::builder()
                .repo_id(options.repo_id)
                .tag(options.tag)
                .get_args(options.get_args.into())
                .build()
                .unwrap(),
        )
    }
}

impl From<ListDockerImages> for DockerOptions {
    fn from(options: ListDockerImages) -> Self {
        DockerOptions::List(
            DockerListCliArgs::builder()
                .repos(options.repos)
                .tags(options.tags)
                .repo_id(options.repo_id)
                .list_args(options.list_args.into())
                .build()
                .unwrap(),
        )
    }
}

pub enum DockerOptions {
    List(DockerListCliArgs),
    Get(DockerImageCliArgs),
}

#[cfg(test)]
mod test {
    use crate::cli::{Args, Command};

    use super::*;

    #[test]
    fn test_docker_cli_repos() {
        let args = Args::parse_from(vec!["gr", "dk", "list", "--repos"]);
        match args.command {
            Command::Docker(DockerCommand {
                subcommand: DockerSubCommand::List(options),
            }) => {
                assert!(options.repos);
                assert!(!options.tags);
            }
            _ => panic!("Expected DockerCommand"),
        }
    }

    #[test]
    fn test_docker_cli_tags() {
        let args = Args::parse_from(vec!["gr", "dk", "list", "--tags", "--repo-id", "12"]);
        match args.command {
            Command::Docker(DockerCommand {
                subcommand: DockerSubCommand::List(options),
            }) => {
                assert!(!options.repos);
                assert!(options.tags);
                assert_eq!(options.repo_id, Some(12));
            }
            _ => panic!("Expected DockerCommand"),
        }
    }

    #[test]
    fn test_docker_get_image_metadata_cli_args() {
        let args = Args::parse_from(vec![
            "gr",
            "dk",
            "image",
            "--refresh",
            "--no-headers",
            "--repo-id",
            "123",
            "v0.0.1",
        ]);
        match args.command {
            Command::Docker(DockerCommand {
                subcommand: DockerSubCommand::Image(options),
            }) => {
                assert_eq!(options.repo_id, 123);
                assert_eq!(options.tag, "v0.0.1");
                assert!(options.get_args.refresh);
                assert!(options.get_args.no_headers);
            }
            _ => panic!("Expected DockerCommand"),
        }
    }
}
