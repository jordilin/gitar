use std::{io::Write, sync::Arc};

use crate::{
    api_traits::{ContainerRegistry, Timestamp},
    cli::docker::DockerOptions,
    config::ConfigProperties,
    display::{self, Column, DisplayBody},
    remote::{self, get_registry, CacheType, GetRemoteCliArgs, ListBodyArgs, ListRemoteCliArgs},
    Result,
};

use super::common::{process_num_metadata, MetadataName};

#[derive(Builder)]
pub struct DockerListCliArgs {
    // If set, list all remote repositories in project's registry
    pub repos: bool,
    // If set, list all tags for a repository
    pub tags: bool,
    pub repo_id: Option<i64>,
    pub list_args: ListRemoteCliArgs,
}

impl DockerListCliArgs {
    pub fn builder() -> DockerListCliArgsBuilder {
        DockerListCliArgsBuilder::default()
    }
}

#[derive(Builder)]
pub struct DockerListBodyArgs {
    #[builder(default)]
    pub repos: bool,
    #[builder(default)]
    pub tags: bool,
    #[builder(default)]
    pub repo_id: Option<i64>,
    #[builder(default)]
    pub body_args: Option<ListBodyArgs>,
}

impl DockerListBodyArgs {
    pub fn builder() -> DockerListBodyArgsBuilder {
        DockerListBodyArgsBuilder::default()
    }
}

#[derive(Builder, Clone)]
pub struct RegistryRepository {
    pub id: i64,
    pub location: String,
    pub tags_count: i64,
    pub created_at: String,
}

impl RegistryRepository {
    pub fn builder() -> RegistryRepositoryBuilder {
        RegistryRepositoryBuilder::default()
    }
}

impl From<RegistryRepository> for DisplayBody {
    fn from(repo: RegistryRepository) -> DisplayBody {
        DisplayBody::new(vec![
            Column::new("ID", repo.id.to_string()),
            Column::new("Location", repo.location),
            Column::new("Tags count", repo.tags_count.to_string()),
            Column::new("Created at", repo.created_at),
        ])
    }
}

impl Timestamp for RegistryRepository {
    fn created_at(&self) -> String {
        self.created_at.clone()
    }
}

#[derive(Builder, Clone)]
pub struct RepositoryTag {
    pub name: String,
    pub path: String,
    pub location: String,
    pub created_at: String,
}

impl RepositoryTag {
    pub fn builder() -> RepositoryTagBuilder {
        RepositoryTagBuilder::default()
    }
}

impl Timestamp for RepositoryTag {
    fn created_at(&self) -> String {
        self.created_at.clone()
    }
}

impl From<RepositoryTag> for DisplayBody {
    fn from(tag: RepositoryTag) -> DisplayBody {
        DisplayBody::new(vec![
            Column::new("Name", tag.name),
            Column::new("Path", tag.path),
            Column::new("Location", tag.location),
        ])
    }
}

#[derive(Builder)]
pub struct DockerImageCliArgs {
    pub tag: String,
    pub repo_id: i64,
    pub get_args: GetRemoteCliArgs,
}

impl DockerImageCliArgs {
    pub fn builder() -> DockerImageCliArgsBuilder {
        DockerImageCliArgsBuilder::default()
    }
}

#[derive(Builder, Clone)]
pub struct ImageMetadata {
    pub name: String,
    pub location: String,
    pub short_sha: String,
    pub size: i64,
    pub created_at: String,
}

impl ImageMetadata {
    pub fn builder() -> ImageMetadataBuilder {
        ImageMetadataBuilder::default()
    }
}

impl From<ImageMetadata> for DisplayBody {
    fn from(metadata: ImageMetadata) -> DisplayBody {
        DisplayBody::new(vec![
            Column::new("Name", metadata.name),
            Column::new("Location", metadata.location),
            Column::new("Short SHA", metadata.short_sha),
            Column::new("Size", metadata.size.to_string()),
            Column::new("Created at", metadata.created_at),
        ])
    }
}

pub fn execute(
    options: DockerOptions,
    config: Arc<dyn ConfigProperties>,
    domain: String,
    path: String,
) -> Result<()> {
    match options {
        DockerOptions::List(cli_args) => {
            let remote = get_registry(
                domain,
                path,
                config,
                Some(&cli_args.list_args.get_args.cache_args),
                CacheType::File,
            )?;
            validate_and_list(remote, cli_args, std::io::stdout())
        }
        DockerOptions::Get(cli_args) => {
            let remote = get_registry(
                domain,
                path,
                config,
                Some(&cli_args.get_args.cache_args),
                CacheType::File,
            )?;
            get_image_metadata(remote, cli_args, std::io::stdout())
        }
    }
}

fn get_image_metadata<W: Write>(
    remote: Arc<dyn ContainerRegistry + Send + Sync>,
    cli_args: DockerImageCliArgs,
    mut writer: W,
) -> Result<()> {
    let metadata = remote.get_image_metadata(cli_args.repo_id, &cli_args.tag)?;
    display::print(&mut writer, vec![metadata], cli_args.get_args)?;
    Ok(())
}

fn validate_and_list<W: Write>(
    remote: Arc<dyn ContainerRegistry + Send + Sync>,
    cli_args: DockerListCliArgs,
    mut writer: W,
) -> Result<()> {
    if cli_args.list_args.num_pages {
        return get_num_pages(remote, cli_args, writer);
    }
    if cli_args.list_args.num_resources {
        return get_num_resources(remote, cli_args, writer);
    }
    let body_args = remote::validate_from_to_page(&cli_args.list_args)?;
    let body_args = DockerListBodyArgs::builder()
        .repos(cli_args.repos)
        .tags(cli_args.tags)
        .repo_id(cli_args.repo_id)
        .body_args(body_args)
        .build()?;
    if body_args.tags {
        let tags = remote.list_repository_tags(body_args)?;
        display::print(&mut writer, tags, cli_args.list_args.get_args)?;
        return Ok(());
    }
    let repos = remote.list_repositories(body_args)?;
    display::print(&mut writer, repos, cli_args.list_args.get_args)
}

fn get_num_pages<W: Write>(
    remote: Arc<dyn ContainerRegistry + Send + Sync>,
    cli_args: DockerListCliArgs,
    writer: W,
) -> Result<()> {
    if cli_args.tags {
        let result = remote.num_pages_repository_tags(cli_args.repo_id.unwrap());
        return process_num_metadata(result, MetadataName::Pages, writer);
    }
    let result = remote.num_pages_repositories();
    process_num_metadata(result, MetadataName::Pages, writer)
}

fn get_num_resources<W: Write>(
    remote: Arc<dyn ContainerRegistry + Send + Sync>,
    cli_args: DockerListCliArgs,
    writer: W,
) -> Result<()> {
    if cli_args.tags {
        let result = remote.num_resources_repository_tags(cli_args.repo_id.unwrap());
        return process_num_metadata(result, MetadataName::Resources, writer);
    }
    let result = remote.num_resources_repositories();
    process_num_metadata(result, MetadataName::Resources, writer)
}

#[cfg(test)]
mod tests {
    use remote::CacheCliArgs;

    use crate::error;

    use super::*;

    #[derive(Builder, Default)]
    struct MockContainerRegistry {
        #[builder(default)]
        num_pages_repos_ok_none: bool,
        #[builder(default)]
        num_pages_repos_err: bool,
    }

    impl MockContainerRegistry {
        pub fn new() -> MockContainerRegistry {
            MockContainerRegistry::default()
        }
        pub fn builder() -> MockContainerRegistryBuilder {
            MockContainerRegistryBuilder::default()
        }
    }

    impl ContainerRegistry for MockContainerRegistry {
        fn list_repositories(&self, _args: DockerListBodyArgs) -> Result<Vec<RegistryRepository>> {
            let repo = RegistryRepository::builder()
                .id(1)
                .location("registry.gitlab.com/namespace/project".to_string())
                .tags_count(10)
                .created_at("2021-01-01T00:00:00Z".to_string())
                .build()
                .unwrap();
            Ok(vec![repo])
        }

        fn list_repository_tags(&self, _args: DockerListBodyArgs) -> Result<Vec<RepositoryTag>> {
            let tag = RepositoryTag::builder()
                .name("v0.0.1".to_string())
                .path("namespace/project:v0.0.1".to_string())
                .location("registry.gitlab.com/namespace/project:v0.0.1".to_string())
                .created_at("2021-01-01T00:00:00Z".to_string())
                .build()
                .unwrap();
            Ok(vec![tag])
        }

        fn num_pages_repository_tags(&self, _repository_id: i64) -> Result<Option<u32>> {
            Ok(Some(3))
        }

        fn num_pages_repositories(&self) -> Result<Option<u32>> {
            if self.num_pages_repos_ok_none {
                return Ok(None);
            }
            if self.num_pages_repos_err {
                return Err(error::gen("Error"));
            }
            Ok(Some(1))
        }

        fn get_image_metadata(&self, _repository_id: i64, tag: &str) -> Result<ImageMetadata> {
            let metadata = ImageMetadata::builder()
                .name(tag.to_string())
                .location(format!("registry.gitlab.com/namespace/project:{tag}"))
                .short_sha("12345678".to_string())
                .size(100)
                .created_at("2021-01-01T00:00:00Z".to_string())
                .build()
                .unwrap();
            Ok(metadata)
        }

        fn num_resources_repository_tags(
            &self,
            _repository_id: i64,
        ) -> Result<Option<crate::api_traits::NumberDeltaErr>> {
            todo!()
        }

        fn num_resources_repositories(&self) -> Result<Option<crate::api_traits::NumberDeltaErr>> {
            todo!()
        }
    }

    #[test]
    fn test_execute_list_repositories() {
        let remote = Arc::new(MockContainerRegistry::new());
        let args = DockerListCliArgs::builder()
            .repos(true)
            .tags(false)
            .repo_id(None)
            .list_args(
                ListRemoteCliArgs::builder()
                    .get_args(
                        GetRemoteCliArgs::builder()
                            .cache_args(CacheCliArgs::default())
                            .build()
                            .unwrap(),
                    )
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        let mut buf = Vec::new();
        validate_and_list(remote, args, &mut buf).unwrap();
        assert_eq!(
            "ID|Location|Tags count|Created at\n\
             1|registry.gitlab.com/namespace/project|10|2021-01-01T00:00:00Z\n",
            String::from_utf8(buf).unwrap()
        );
    }

    #[test]
    fn test_execute_list_tags() {
        let remote = Arc::new(MockContainerRegistry::new());
        let args = DockerListCliArgs::builder()
            .repos(false)
            .tags(true)
            .repo_id(Some(1))
            .list_args(
                ListRemoteCliArgs::builder()
                    .get_args(
                        GetRemoteCliArgs::builder()
                            .cache_args(CacheCliArgs::default())
                            .build()
                            .unwrap(),
                    )
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        let mut buf = Vec::new();
        validate_and_list(remote, args, &mut buf).unwrap();
        assert_eq!(
            "Name|Path|Location\n\
            v0.0.1|namespace/project:v0.0.1|registry.gitlab.com/namespace/project:v0.0.1\n",
            String::from_utf8(buf).unwrap()
        );
    }

    #[test]
    fn test_get_num_pages_for_listing_tags() {
        let remote = Arc::new(MockContainerRegistry::new());
        let args = DockerListCliArgs::builder()
            .repos(false)
            .tags(true)
            .repo_id(Some(1))
            .list_args(
                ListRemoteCliArgs::builder()
                    .get_args(
                        GetRemoteCliArgs::builder()
                            .cache_args(CacheCliArgs::default())
                            .build()
                            .unwrap(),
                    )
                    .num_pages(true)
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        let mut buf = Vec::new();
        validate_and_list(remote, args, &mut buf).unwrap();
        assert_eq!("3\n", String::from_utf8(buf).unwrap());
    }

    #[test]
    fn test_get_num_pages_for_listing_repositories() {
        let remote = Arc::new(MockContainerRegistry::new());
        let args = DockerListCliArgs::builder()
            .repos(true)
            .tags(false)
            .repo_id(None)
            .list_args(
                ListRemoteCliArgs::builder()
                    .get_args(
                        GetRemoteCliArgs::builder()
                            .cache_args(CacheCliArgs::default())
                            .build()
                            .unwrap(),
                    )
                    .num_pages(true)
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        let mut buf = Vec::new();
        validate_and_list(remote, args, &mut buf).unwrap();
        assert_eq!("1\n", String::from_utf8(buf).unwrap());
    }

    #[test]
    fn test_do_not_print_headers_if_no_headers_provided_for_tags() {
        let remote = Arc::new(MockContainerRegistry::new());
        let args = DockerListCliArgs::builder()
            .repos(false)
            .tags(true)
            .repo_id(Some(1))
            .list_args(
                ListRemoteCliArgs::builder()
                    .get_args(
                        GetRemoteCliArgs::builder()
                            .cache_args(CacheCliArgs::default())
                            .no_headers(true)
                            .build()
                            .unwrap(),
                    )
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        let mut buf = Vec::new();
        validate_and_list(remote, args, &mut buf).unwrap();
        assert_eq!(
            "v0.0.1|namespace/project:v0.0.1|registry.gitlab.com/namespace/project:v0.0.1\n",
            String::from_utf8(buf).unwrap()
        );
    }

    #[test]
    fn test_do_not_print_headers_if_no_headers_provided_for_repositories() {
        let remote = Arc::new(MockContainerRegistry::new());
        let args = DockerListCliArgs::builder()
            .repos(true)
            .tags(false)
            .repo_id(None)
            .list_args(
                ListRemoteCliArgs::builder()
                    .get_args(
                        GetRemoteCliArgs::builder()
                            .cache_args(CacheCliArgs::default())
                            .no_headers(true)
                            .build()
                            .unwrap(),
                    )
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        let mut buf = Vec::new();
        validate_and_list(remote, args, &mut buf).unwrap();
        assert_eq!(
            "1|registry.gitlab.com/namespace/project|10|2021-01-01T00:00:00Z\n",
            String::from_utf8(buf).unwrap()
        );
    }

    #[test]
    fn test_num_pages_not_available_in_headers() {
        let remote = Arc::new(
            MockContainerRegistry::builder()
                .num_pages_repos_ok_none(true)
                .build()
                .unwrap(),
        );
        let args = DockerListCliArgs::builder()
            .repos(true)
            .tags(false)
            .repo_id(None)
            .list_args(
                ListRemoteCliArgs::builder()
                    .get_args(
                        GetRemoteCliArgs::builder()
                            .cache_args(CacheCliArgs::default())
                            .no_headers(true)
                            .build()
                            .unwrap(),
                    )
                    .num_pages(true)
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        let mut buf = Vec::new();
        validate_and_list(remote, args, &mut buf).unwrap();
        assert_eq!(
            "Number of pages not available.\n",
            String::from_utf8(buf).unwrap()
        );
    }

    #[test]
    fn test_num_pages_error_in_remote_is_error() {
        let remote = Arc::new(
            MockContainerRegistry::builder()
                .num_pages_repos_err(true)
                .build()
                .unwrap(),
        );
        let args = DockerListCliArgs::builder()
            .repos(true)
            .tags(false)
            .repo_id(None)
            .list_args(
                ListRemoteCliArgs::builder()
                    .get_args(
                        GetRemoteCliArgs::builder()
                            .cache_args(CacheCliArgs::default())
                            .no_headers(true)
                            .build()
                            .unwrap(),
                    )
                    .num_pages(true)
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        let mut buf = Vec::new();
        assert!(validate_and_list(remote, args, &mut buf).is_err());
    }

    #[test]
    fn test_get_image_metadata() {
        let remote = Arc::new(MockContainerRegistry::new());
        let args = DockerImageCliArgs::builder()
            .tag("v0.0.1".to_string())
            .repo_id(1)
            .get_args(GetRemoteCliArgs::builder().build().unwrap())
            .build()
            .unwrap();
        let mut buf = Vec::new();
        get_image_metadata(remote, args, &mut buf).unwrap();
        assert_eq!(
            "Name|Location|Short SHA|Size|Created at\n\
            v0.0.1|registry.gitlab.com/namespace/project:v0.0.1|12345678|100|2021-01-01T00:00:00Z\n",
            String::from_utf8(buf).unwrap()
        );
    }

    #[test]
    fn test_get_image_metadata_no_headers() {
        let remote = Arc::new(MockContainerRegistry::new());
        let args = DockerImageCliArgs::builder()
            .tag("v0.0.1".to_string())
            .repo_id(1)
            .get_args(
                GetRemoteCliArgs::builder()
                    .cache_args(CacheCliArgs::default())
                    .no_headers(true)
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        let mut buf = Vec::new();
        get_image_metadata(remote, args, &mut buf).unwrap();
        assert_eq!(
            "v0.0.1|registry.gitlab.com/namespace/project:v0.0.1|12345678|100|2021-01-01T00:00:00Z\n",
            String::from_utf8(buf).unwrap()
        );
    }
}
