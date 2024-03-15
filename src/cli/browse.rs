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

pub enum BrowseOptions {
    // defaults to open repo in browser
    Repo,
    MergeRequests,
    MergeRequestId(i64),
    Pipelines,
}

#[derive(Parser)]
struct MergeRequestBrowse {
    /// Open merge/pull request id in the browser
    #[clap()]
    pub id: Option<i64>,
}
