use std::fmt::Display;

use crate::{
    cli::BrowseOptions,
    io::CmdInfo,
    remote::{
        MergeRequestBodyArgs, MergeRequestListBodyArgs, MergeRequestResponse, Pipeline,
        PipelineBodyArgs,
    },
    Result,
};

pub trait MergeRequest {
    fn open(&self, args: MergeRequestBodyArgs) -> Result<MergeRequestResponse>;
    fn list(&self, args: MergeRequestListBodyArgs) -> Result<Vec<MergeRequestResponse>>;
    fn merge(&self, id: i64) -> Result<MergeRequestResponse>;
    fn get(&self, id: i64) -> Result<MergeRequestResponse>;
    fn close(&self, id: i64) -> Result<MergeRequestResponse>;
    /// Queries the remote API to get the number of pages available for a given
    /// resource based on list arguments.
    fn num_pages(&self, args: MergeRequestListBodyArgs) -> Result<Option<u32>>;
}

pub trait RemoteProject {
    fn get_project_data(&self, id: Option<i64>) -> Result<CmdInfo>;
    fn get_project_members(&self) -> Result<CmdInfo>;
    // User requests to open a browser using the remote url. It can open the
    // merge/pull requests, pipeline, issues, etc.
    fn get_url(&self, option: BrowseOptions) -> String;
}

pub trait Cicd {
    fn list(&self, args: PipelineBodyArgs) -> Result<Vec<Pipeline>>;
    fn get_pipeline(&self, id: i64) -> Result<Pipeline>;
    fn num_pages(&self) -> Result<Option<u32>>;
}

pub trait Timestamp {
    fn created_at(&self) -> String;
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

impl Display for ApiOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiOperation::MergeRequest => write!(f, "merge_request"),
            ApiOperation::Pipeline => write!(f, "pipeline"),
            ApiOperation::Project => write!(f, "project"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_operation_display() {
        assert_eq!(format!("{}", ApiOperation::MergeRequest), "merge_request");
        assert_eq!(format!("{}", ApiOperation::Pipeline), "pipeline");
        assert_eq!(format!("{}", ApiOperation::Project), "project");
    }
}
