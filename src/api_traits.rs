use std::{fmt::Display, str::FromStr};

use serde::Deserialize;

use crate::{
    cli::browse::BrowseOptions,
    cmds::{
        cicd::{
            Job, JobListBodyArgs, LintResponse, Pipeline, PipelineBodyArgs, Runner,
            RunnerListBodyArgs, RunnerMetadata, RunnerPostDataCliArgs, RunnerRegistrationResponse,
            YamlBytes,
        },
        docker::{DockerListBodyArgs, ImageMetadata, RegistryRepository, RepositoryTag},
        gist::{Gist, GistListBodyArgs},
        merge_request::{
            Comment, CommentMergeRequestBodyArgs, CommentMergeRequestListBodyArgs,
            MergeRequestBodyArgs, MergeRequestListBodyArgs, MergeRequestResponse,
        },
        project::{Member, Project, ProjectListBodyArgs, Tag},
        release::{Release, ReleaseAssetListBodyArgs, ReleaseAssetMetadata, ReleaseBodyArgs},
        trending::TrendingProject,
        user::UserCliArgs,
    },
    io::CmdInfo,
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
    fn num_resources(&self, args: MergeRequestListBodyArgs) -> Result<Option<NumberDeltaErr>>;
}

pub trait RemoteProject {
    /// Get the project data from the remote API. Implementers will need to pass
    /// either an `id` or a `path`. The `path` should be in the format
    /// `OWNER/PROJECT_NAME`
    fn get_project_data(&self, id: Option<i64>, path: Option<&str>) -> Result<CmdInfo>;
    fn get_project_members(&self) -> Result<CmdInfo>;
    /// User requests to open a browser using the remote url. It can open the
    /// merge/pull requests, pipeline, issues, etc.
    fn get_url(&self, option: BrowseOptions) -> String;
    fn list(&self, args: ProjectListBodyArgs) -> Result<Vec<Project>>;
    fn num_pages(&self, args: ProjectListBodyArgs) -> Result<Option<u32>>;
    fn num_resources(&self, args: ProjectListBodyArgs) -> Result<Option<NumberDeltaErr>>;
}

pub trait RemoteTag: RemoteProject {
    fn list(&self, args: ProjectListBodyArgs) -> Result<Vec<Tag>>;
}

pub trait ProjectMember: RemoteProject {
    fn list(&self, args: ProjectListBodyArgs) -> Result<Vec<Member>>;
}

pub trait Cicd {
    fn list(&self, args: PipelineBodyArgs) -> Result<Vec<Pipeline>>;
    fn get_pipeline(&self, id: i64) -> Result<Pipeline>;
    fn num_pages(&self) -> Result<Option<u32>>;
    fn num_resources(&self) -> Result<Option<NumberDeltaErr>>;
    /// Lints ci/cd pipeline file contents. In gitlab this is the .gitlab-ci.yml
    /// file. Checks that the file is valid and has no syntax errors.
    fn lint(&self, body: YamlBytes) -> Result<LintResponse>;
}

pub trait CicdRunner {
    fn list(&self, args: RunnerListBodyArgs) -> Result<Vec<Runner>>;
    fn get(&self, id: i64) -> Result<RunnerMetadata>;
    fn create(&self, args: RunnerPostDataCliArgs) -> Result<RunnerRegistrationResponse>;
    fn num_pages(&self, args: RunnerListBodyArgs) -> Result<Option<u32>>;
    fn num_resources(&self, args: RunnerListBodyArgs) -> Result<Option<NumberDeltaErr>>;
}

pub trait CicdJob {
    fn list(&self, args: JobListBodyArgs) -> Result<Vec<Job>>;
    fn num_pages(&self, args: JobListBodyArgs) -> Result<Option<u32>>;
    fn num_resources(&self, args: JobListBodyArgs) -> Result<Option<NumberDeltaErr>>;
}

pub trait Deploy {
    fn list(&self, args: ReleaseBodyArgs) -> Result<Vec<Release>>;
    fn num_pages(&self) -> Result<Option<u32>>;
    fn num_resources(&self) -> Result<Option<NumberDeltaErr>>;
}

pub trait DeployAsset {
    fn list(&self, args: ReleaseAssetListBodyArgs) -> Result<Vec<ReleaseAssetMetadata>>;
    fn num_pages(&self, args: ReleaseAssetListBodyArgs) -> Result<Option<u32>>;
    fn num_resources(&self, args: ReleaseAssetListBodyArgs) -> Result<Option<NumberDeltaErr>>;
}

pub trait UserInfo {
    /// Get the user's information from the remote API.
    fn get_auth_user(&self) -> Result<Member>;
    fn get(&self, args: &UserCliArgs) -> Result<Member>;
}

pub trait CodeGist {
    fn list(&self, args: GistListBodyArgs) -> Result<Vec<Gist>>;
    fn num_pages(&self) -> Result<Option<u32>>;
    fn num_resources(&self) -> Result<Option<NumberDeltaErr>>;
}

pub trait Timestamp {
    fn created_at(&self) -> String;
}

pub trait ContainerRegistry {
    fn list_repositories(&self, args: DockerListBodyArgs) -> Result<Vec<RegistryRepository>>;
    fn list_repository_tags(&self, args: DockerListBodyArgs) -> Result<Vec<RepositoryTag>>;
    fn num_pages_repository_tags(&self, repository_id: i64) -> Result<Option<u32>>;
    fn num_resources_repository_tags(&self, repository_id: i64) -> Result<Option<NumberDeltaErr>>;
    fn num_pages_repositories(&self) -> Result<Option<u32>>;
    fn num_resources_repositories(&self) -> Result<Option<NumberDeltaErr>>;
    fn get_image_metadata(&self, repository_id: i64, tag: &str) -> Result<ImageMetadata>;
}

pub trait CommentMergeRequest {
    fn create(&self, args: CommentMergeRequestBodyArgs) -> Result<()>;
    fn list(&self, args: CommentMergeRequestListBodyArgs) -> Result<Vec<Comment>>;
    fn num_pages(&self, args: CommentMergeRequestListBodyArgs) -> Result<Option<u32>>;
    fn num_resources(
        &self,
        args: CommentMergeRequestListBodyArgs,
    ) -> Result<Option<NumberDeltaErr>>;
}

pub trait TrendingProjectURL {
    fn list(&self, language: String) -> Result<Vec<TrendingProject>>;
}

/// Represents a type carrying a result and a delta error. This is the case when
/// querying the number of resources such as releases, pipelines, etc...
/// available. REST APIs don't carry a count, so that is computed by the total
/// number of pages available (last page in link header) and the number of
/// resources per page.
pub struct NumberDeltaErr {
    /// Possible number of resources = num_pages * resources_per_page
    pub num: u32,
    /// Resources per_page
    pub delta: u32,
}

impl NumberDeltaErr {
    pub fn new(num: u32, delta: u32) -> Self {
        Self { num, delta }
    }

    fn compute_interval(&self) -> (u32, u32) {
        if self.num < self.delta {
            return (1, self.delta);
        }
        (self.num - self.delta + 1, self.num)
    }
}

impl Display for NumberDeltaErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (start, end) = self.compute_interval();
        write!(f, "({start}, {end})")
    }
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
    // Get request to a single URL page. Ex. The trending repositories in github.com
    SinglePage,
    // Gists
    Gist,
    RepositoryTag,
}

impl Display for ApiOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiOperation::MergeRequest => write!(f, "merge_request"),
            ApiOperation::Pipeline => write!(f, "pipeline"),
            ApiOperation::Project => write!(f, "project"),
            ApiOperation::ContainerRegistry => write!(f, "container_registry"),
            ApiOperation::Release => write!(f, "release"),
            ApiOperation::SinglePage => write!(f, "single_page"),
            ApiOperation::Gist => write!(f, "gist"),
            ApiOperation::RepositoryTag => write!(f, "repository_tag"),
        }
    }
}

impl<'de> Deserialize<'de> for ApiOperation {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        ApiOperation::from_str(&s).map_err(serde::de::Error::custom)
    }
}

impl FromStr for ApiOperation {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<ApiOperation, std::string::String> {
        match s.to_lowercase().as_str() {
            "merge_request" => Ok(ApiOperation::MergeRequest),
            "pipeline" => Ok(ApiOperation::Pipeline),
            "project" => Ok(ApiOperation::Project),
            "container_registry" => Ok(ApiOperation::ContainerRegistry),
            "release" => Ok(ApiOperation::Release),
            "single_page" => Ok(ApiOperation::SinglePage),
            "gist" => Ok(ApiOperation::Gist),
            "repository_tag" => Ok(ApiOperation::RepositoryTag),
            _ => Err(format!("Unknown ApiOperation: {s}")),
        }
    }
}

pub struct ApiOperationIterator {
    current: Option<ApiOperation>,
}

impl ApiOperationIterator {
    fn new() -> Self {
        ApiOperationIterator { current: None }
    }
}

impl Iterator for ApiOperationIterator {
    type Item = ApiOperation;

    fn next(&mut self) -> Option<Self::Item> {
        let next = match self.current {
            None => Some(ApiOperation::MergeRequest),
            Some(ApiOperation::MergeRequest) => Some(ApiOperation::Pipeline),
            Some(ApiOperation::Pipeline) => Some(ApiOperation::Project),
            Some(ApiOperation::Project) => Some(ApiOperation::ContainerRegistry),
            Some(ApiOperation::ContainerRegistry) => Some(ApiOperation::Release),
            Some(ApiOperation::Release) => Some(ApiOperation::SinglePage),
            Some(ApiOperation::SinglePage) => Some(ApiOperation::Gist),
            Some(ApiOperation::Gist) => Some(ApiOperation::RepositoryTag),
            Some(ApiOperation::RepositoryTag) => None,
        };
        self.current = next.clone();
        next
    }
}

impl ApiOperation {
    pub fn iter() -> ApiOperationIterator {
        ApiOperationIterator::new()
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
        assert_eq!(format!("{}", ApiOperation::Release), "release");
        assert_eq!(format!("{}", ApiOperation::SinglePage), "single_page");
    }

    #[test]
    fn test_delta_err_display() {
        let delta_err = NumberDeltaErr::new(40, 20);
        assert_eq!("(21, 40)", delta_err.to_string());
    }

    #[test]
    fn test_num_less_than_delta_begins_at_one_up_to_delta() {
        let delta_err = NumberDeltaErr::new(25, 30);
        assert_eq!("(1, 30)", delta_err.to_string());
    }

    #[test]
    fn test_api_operation_iterator() {
        let operations: Vec<ApiOperation> = ApiOperation::iter().collect();
        assert_eq!(operations.len(), 8);
        assert_eq!(operations[0], ApiOperation::MergeRequest);
        assert_eq!(operations[7], ApiOperation::RepositoryTag);
    }
}
