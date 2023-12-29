use crate::{
    cli::BrowseOptions,
    io::CmdInfo,
    remote::{MergeRequestArgs, MergeRequestResponse, MergeRequestState},
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

pub trait Pipeline {
    fn list_pipeline(&self) -> Result<CmdInfo>;
    fn get_pipeline(&self, id: i64) -> Result<CmdInfo>;
}

pub trait Remote: RemoteProject + MergeRequest + Pipeline + Send + Sync + 'static {}
