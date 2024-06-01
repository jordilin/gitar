use clap::Parser;

#[derive(Parser)]
pub struct BrowseCommand {
    #[clap(subcommand)]
    subcommand: Option<BrowseSubcommand>,
}

#[derive(Parser)]
enum BrowseSubcommand {
    #[clap(about = "Open the repo using your browser")]
    Repo,
    #[clap(name = "mr", about = "Open the merge requests using your browser")]
    MergeRequest(MergeRequestBrowse),
    #[clap(name = "pp", about = "Open the ci/cd pipelines using your browser")]
    Pipelines,
}

impl From<MergeRequestBrowse> for BrowseOptions {
    fn from(options: MergeRequestBrowse) -> Self {
        match options.id {
            Some(id) => BrowseOptions::MergeRequestId(id),
            None => BrowseOptions::MergeRequests,
        }
    }
}

impl From<BrowseCommand> for BrowseOptions {
    fn from(options: BrowseCommand) -> Self {
        match options.subcommand {
            Some(BrowseSubcommand::Repo) => BrowseOptions::Repo,
            Some(BrowseSubcommand::MergeRequest(options)) => options.into(),
            Some(BrowseSubcommand::Pipelines) => BrowseOptions::Pipelines,
            // defaults to open repo in browser
            None => BrowseOptions::Repo,
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum BrowseOptions {
    // defaults to open repo in browser
    Repo,
    MergeRequests,
    MergeRequestId(i64),
    Pipelines,
    Manual,
}

#[derive(Parser)]
struct MergeRequestBrowse {
    /// Open merge/pull request id in the browser
    #[clap()]
    pub id: Option<i64>,
}

#[cfg(test)]
mod test {

    use super::*;
    use crate::cli::{Args, Command};

    #[test]
    fn test_browse_command_repo() {
        let args = Args::parse_from(vec!["gr", "br", "repo"]);
        match args.command {
            Command::Browse(BrowseCommand {
                subcommand: Some(BrowseSubcommand::Repo),
            }) => {}
            _ => panic!("Expected Repo BrowseCommand"),
        }
    }

    #[test]
    fn test_browse_command_mr() {
        let args = Args::parse_from(vec!["gr", "br", "mr"]);
        let mr_browse = match args.command {
            Command::Browse(BrowseCommand {
                subcommand: Some(BrowseSubcommand::MergeRequest(options)),
            }) => {
                assert_eq!(options.id, None);
                options
            }
            _ => panic!("Expected MergeRequest BrowseCommand"),
        };
        let options: BrowseOptions = mr_browse.into();
        assert_eq!(options, BrowseOptions::MergeRequests);
    }

    #[test]
    fn test_browse_command_mr_id() {
        let args = Args::parse_from(vec!["gr", "br", "mr", "1"]);
        let mr_browse = match args.command {
            Command::Browse(BrowseCommand {
                subcommand: Some(BrowseSubcommand::MergeRequest(options)),
            }) => {
                assert_eq!(options.id, Some(1));
                options
            }
            _ => panic!("Expected MergeRequest BrowseCommand"),
        };
        let options: BrowseOptions = mr_browse.into();
        assert_eq!(options, BrowseOptions::MergeRequestId(1));
    }

    #[test]
    fn test_browse_command_pipelines() {
        let args = Args::parse_from(vec!["gr", "br", "pp"]);
        match args.command {
            Command::Browse(BrowseCommand {
                subcommand: Some(BrowseSubcommand::Pipelines),
            }) => {}
            _ => panic!("Expected Pipelines BrowseCommand"),
        }
    }
}
