use crate::{
    cli::BrowseOptions,
    io::CmdInfo,
    remote::{MergeRequestArgs, MergeRequestResponse, MergeRequestState, Pipeline},
    Result,
};

pub trait MergeRequest {
    fn open(&self, args: MergeRequestArgs) -> Result<MergeRequestResponse>;
    fn list(&self, state: MergeRequestState) -> Result<Vec<MergeRequestResponse>>;
    fn merge(&self, id: i64) -> Result<MergeRequestResponse>;
    fn get(&self, id: i64) -> Result<MergeRequestResponse>;
    fn close(&self, id: i64) -> Result<MergeRequestResponse>;
}

pub trait RemoteProject {
    fn get_project_data(&self) -> Result<CmdInfo>;
    fn get_project_members(&self) -> Result<CmdInfo>;
    // User requests to open a browser using the remote url. It can open the
    // merge/pull requests, pipeline, issues, etc.
    fn get_url(&self, option: BrowseOptions) -> String;
}

pub trait Cicd {
    fn list_pipelines(&self) -> Result<Vec<Pipeline>>;
    fn get_pipeline(&self, id: i64) -> Result<Pipeline>;
}

/// Types of API resources attached to a request. The request will carry this
/// information so we can decide if we need to use the cache or not based on
/// global configuration.
/// This is for read requests only, so that would be list merge_requests, list
/// pipelines, get one merge request information, etc...
#[derive(Clone, Debug, PartialEq, Hash, Eq)]
pub enum ApiOperation {
    MergeRequest,
    Pipeline,
    // Project members, project data such as default upstream branch, project
    // id, etc...any metadata related to the project.
    Project,
}

pub trait Remote: RemoteProject + MergeRequest + Cicd + Send + Sync + 'static {}
