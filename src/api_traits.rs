use std::fmt::Display;

use crate::{
    cli::browse::BrowseOptions,
    cmds::{
        cicd::{Pipeline, PipelineBodyArgs, Runner, RunnerListBodyArgs, RunnerMetadata},
        docker::{DockerListBodyArgs, ImageMetadata, RegistryRepository, RepositoryTag},
        merge_request::{Comment, CommentMergeRequestBodyArgs, CommentMergeRequestListBodyArgs},
        project::ProjectListBodyArgs,
        release::{Release, ReleaseBodyArgs},
    },
    io::CmdInfo,
    remote::{
        Member, MergeRequestBodyArgs, MergeRequestListBodyArgs, MergeRequestResponse, Project,
    },
    Result,
};

pub trait MergeRequest {
    fn open(&self, args: MergeRequestBodyArgs) -> Result<MergeRequestResponse>;
    fn list(&self, args: MergeRequestListBodyArgs) -> Result<Vec<MergeRequestResponse>>;
    fn merge(&self, id: i64) -> Result<MergeRequestResponse>;
    fn get(&self, id: i64) -> Result<MergeRequestResponse>;
    fn close(&self, id: i64) -> Result<MergeRequestResponse>;
    fn approve(&self, id: i64) -> Result<MergeRequestResponse>;
    /// Queries the remote API to get the number of pages available for a given
    /// resource based on list arguments.
    fn num_pages(&self, args: MergeRequestListBodyArgs) -> Result<Option<u32>>;
}

pub trait RemoteProject {
    /// Get the project data from the remote API. Implementors will need to pass
    /// either an `id` or a `path`. The `path` should be in the format
    /// `OWNER/PROJECT_NAME`
    fn get_project_data(&self, id: Option<i64>, path: Option<&str>) -> Result<CmdInfo>;
    fn get_project_members(&self) -> Result<CmdInfo>;
    /// User requests to open a browser using the remote url. It can open the
    /// merge/pull requests, pipeline, issues, etc.
    fn get_url(&self, option: BrowseOptions) -> String;
    fn list(&self, args: ProjectListBodyArgs) -> Result<Vec<Project>>;
    fn num_pages(&self, args: ProjectListBodyArgs) -> Result<Option<u32>>;
}

pub trait Cicd {
    fn list(&self, args: PipelineBodyArgs) -> Result<Vec<Pipeline>>;
    fn get_pipeline(&self, id: i64) -> Result<Pipeline>;
    fn num_pages(&self) -> Result<Option<u32>>;
}

pub trait CicdRunner {
    fn list(&self, args: RunnerListBodyArgs) -> Result<Vec<Runner>>;
    fn get(&self, id: i64) -> Result<RunnerMetadata>;
    fn num_pages(&self, args: RunnerListBodyArgs) -> Result<Option<u32>>;
}

pub trait Deploy {
    fn list(&self, args: ReleaseBodyArgs) -> Result<Vec<Release>>;
    fn num_pages(&self) -> Result<Option<u32>>;
}

pub trait UserInfo {
    /// Get the user's information from the remote API.
    fn get(&self) -> Result<Member>;
}

pub trait Timestamp {
    fn created_at(&self) -> String;
}

pub trait ContainerRegistry {
    fn list_repositories(&self, args: DockerListBodyArgs) -> Result<Vec<RegistryRepository>>;
    fn list_repository_tags(&self, args: DockerListBodyArgs) -> Result<Vec<RepositoryTag>>;
    fn num_pages_repository_tags(&self, repository_id: i64) -> Result<Option<u32>>;
    fn num_pages_repositories(&self) -> Result<Option<u32>>;
    fn get_image_metadata(&self, repository_id: i64, tag: &str) -> Result<ImageMetadata>;
}

pub trait CommentMergeRequest {
    fn create(&self, args: CommentMergeRequestBodyArgs) -> Result<()>;
    fn list(&self, args: CommentMergeRequestListBodyArgs) -> Result<Vec<Comment>>;
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
    ContainerRegistry,
    Release,
}

impl Display for ApiOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiOperation::MergeRequest => write!(f, "merge_request"),
            ApiOperation::Pipeline => write!(f, "pipeline"),
            ApiOperation::Project => write!(f, "project"),
            ApiOperation::ContainerRegistry => write!(f, "container_registry"),
            ApiOperation::Release => write!(f, "release"),
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
        assert_eq!(
            format!("{}", ApiOperation::ContainerRegistry),
            "container_registry"
        );
    }
}
