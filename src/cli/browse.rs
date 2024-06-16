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
    Pipelines(PipelineBrowse),
    #[clap(name = "rl", about = "Open the releases page using your browser")]
    Release,
}

impl From<MergeRequestBrowse> for BrowseOptions {
    fn from(options: MergeRequestBrowse) -> Self {
        match options.id {
            Some(id) => BrowseOptions::MergeRequestId(id),
            None => BrowseOptions::MergeRequests,
        }
    }
}

impl From<PipelineBrowse> for BrowseOptions {
    fn from(options: PipelineBrowse) -> Self {
        match options.id {
            Some(id) => BrowseOptions::PipelineId(id),
            None => BrowseOptions::Pipelines,
        }
    }
}

impl From<BrowseCommand> for BrowseOptions {
    fn from(options: BrowseCommand) -> Self {
        match options.subcommand {
            Some(BrowseSubcommand::Repo) => BrowseOptions::Repo,
            Some(BrowseSubcommand::MergeRequest(options)) => options.into(),
            Some(BrowseSubcommand::Pipelines(options)) => options.into(),
            Some(BrowseSubcommand::Release) => BrowseOptions::Releases,
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
    PipelineId(i64),
    Releases,
    Manual,
}

#[derive(Parser)]
struct MergeRequestBrowse {
    /// Open merge/pull request id in the browser
    #[clap()]
    pub id: Option<i64>,
}

#[derive(Parser)]
struct PipelineBrowse {
    /// Open pipeline id in the browser
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
        let pp_browse = match args.command {
            Command::Browse(BrowseCommand {
                subcommand: Some(BrowseSubcommand::Pipelines(options)),
            }) => {
                assert_eq!(options.id, None);
                options
            }
            _ => panic!("Expected Pipelines BrowseCommand"),
        };
        let options: BrowseOptions = pp_browse.into();
        assert_eq!(options, BrowseOptions::Pipelines);
    }

    #[test]
    fn test_browse_command_pp_id() {
        let args = Args::parse_from(vec!["gr", "br", "pp", "1"]);
        let mr_browse = match args.command {
            Command::Browse(BrowseCommand {
                subcommand: Some(BrowseSubcommand::Pipelines(options)),
            }) => {
                assert_eq!(options.id, Some(1));
                options
            }
            _ => panic!("Expected Pipeline BrowseCommand"),
        };
        let options: BrowseOptions = mr_browse.into();
        assert_eq!(options, BrowseOptions::PipelineId(1));
    }
}
