use std::{
    fmt::{self, Display, Formatter},
    io::Write,
    sync::Arc,
};

use crate::{
    api_traits::{ContainerRegistry, Timestamp},
    cli::DockerOptions,
    config::Config,
    remote::{self, get_registry, ListBodyArgs, ListRemoteCliArgs},
    Result,
};

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

#[derive(Builder)]
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

impl Display for RegistryRepository {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} | {} | {} | {}",
            self.id, self.location, self.tags_count, self.created_at
        )
    }
}

impl Timestamp for RegistryRepository {
    fn created_at(&self) -> String {
        self.created_at.clone()
    }
}

#[derive(Builder)]
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

impl Display for RepositoryTag {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{} | {} | {}", self.name, self.path, self.location)
    }
}

#[derive(Builder)]
pub struct DockerImageCliArgs {
    pub tag: String,
    pub repo_id: i64,
    pub refresh_cache: bool,
    pub no_headers: bool,
}

impl DockerImageCliArgs {
    pub fn builder() -> DockerImageCliArgsBuilder {
        DockerImageCliArgsBuilder::default()
    }
}

#[derive(Builder)]
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

impl Display for ImageMetadata {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} | {} | {} | {} | {}",
            self.name, self.location, self.short_sha, self.size, self.created_at
        )
    }
}

pub fn execute(
    options: DockerOptions,
    config: Arc<Config>,
    domain: String,
    path: String,
) -> Result<()> {
    match options {
        DockerOptions::List(cli_args) => {
            let remote = get_registry(domain, path, config, cli_args.list_args.refresh_cache)?;
            validate_and_list(remote, cli_args, std::io::stdout())
        }
        DockerOptions::Get(cli_args) => {
            let remote = get_registry(domain, path, config, cli_args.refresh_cache)?;
            get_image_metadata(remote, cli_args, std::io::stdout())
        }
    }
}

fn get_image_metadata<W: Write>(
    remote: Arc<dyn ContainerRegistry + Send + Sync>,
    cli_args: DockerImageCliArgs,
    mut writer: W,
) -> Result<()> {
    let metadata = remote.get_image_metadata(cli_args.repo_id, cli_args.tag)?;
    if cli_args.no_headers {
        writer.write_all(format!("{}\n", metadata).as_bytes())?;
    } else {
        let headers = "Name | Location | Short SHA | Size | Created at\n";
        writer.write_all(headers.as_bytes())?;
        writer.write_all(format!("{}\n", metadata).as_bytes())?;
    }
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
    let body_args = remote::validate_from_to_page(&cli_args.list_args)?;
    let body_args = DockerListBodyArgs::builder()
        .repos(cli_args.repos)
        .tags(cli_args.tags)
        .repo_id(cli_args.repo_id)
        .body_args(body_args)
        .build()?;
    if body_args.tags {
        if cli_args.list_args.no_headers {
            return list_repository_tags(remote, body_args, writer);
        }
        let headers = "Name | Path | Location\n";
        writer.write_all(headers.as_bytes())?;
        return list_repository_tags(remote, body_args, writer);
    }
    if cli_args.list_args.no_headers {
        return list_repositories(remote, body_args, writer);
    }
    let headers = "ID | Location | Tags count | Created at\n";
    writer.write_all(headers.as_bytes())?;
    list_repositories(remote, body_args, writer)
}

fn get_num_pages<W: Write>(
    remote: Arc<dyn ContainerRegistry + Send + Sync>,
    cli_args: DockerListCliArgs,
    writer: W,
) -> Result<()> {
    if cli_args.tags {
        let result = remote.num_pages_repository_tags(cli_args.repo_id.unwrap());
        return report_num_pages(result, writer);
    }
    let result = remote.num_pages_repositories();
    report_num_pages(result, writer)
}

fn report_num_pages<W: Write>(pages: Result<Option<u32>>, mut writer: W) -> Result<()> {
    match pages {
        Ok(Some(pages)) => writer.write_all(format!("{pages}\n", pages = pages).as_bytes())?,
        Ok(None) => {
            writer.write_all(b"Number of pages not available.\n")?;
        }
        Err(e) => {
            return Err(e);
        }
    }
    Ok(())
}

pub fn list_repository_tags<W: Write>(
    remote: Arc<dyn ContainerRegistry>,
    args: DockerListBodyArgs,
    mut writer: W,
) -> Result<()> {
    for tag in remote.list_repository_tags(args)? {
        writer.write_all(format!("{}\n", tag).as_bytes())?;
    }
    Ok(())
}

pub fn list_repositories<W: Write>(
    remote: Arc<dyn ContainerRegistry>,
    args: DockerListBodyArgs,
    mut writer: W,
) -> Result<()> {
    for repo in remote.list_repositories(args)? {
        writer.write_all(format!("{}\n", repo).as_bytes())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
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

        fn get_image_metadata(&self, _repository_id: i64, tag: String) -> Result<ImageMetadata> {
            let metadata = ImageMetadata::builder()
                .name(tag.clone())
                .location(format!("registry.gitlab.com/namespace/project:{}", tag))
                .short_sha("12345678".to_string())
                .size(100)
                .created_at("2021-01-01T00:00:00Z".to_string())
                .build()
                .unwrap();
            Ok(metadata)
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
                    .refresh_cache(false)
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        let mut buf = Vec::new();
        validate_and_list(remote, args, &mut buf).unwrap();
        assert_eq!(
            "ID | Location | Tags count | Created at\n\
             1 | registry.gitlab.com/namespace/project | 10 | 2021-01-01T00:00:00Z\n",
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
                    .refresh_cache(false)
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        let mut buf = Vec::new();
        validate_and_list(remote, args, &mut buf).unwrap();
        assert_eq!(
            "Name | Path | Location\n\
            v0.0.1 | namespace/project:v0.0.1 | registry.gitlab.com/namespace/project:v0.0.1\n",
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
                    .refresh_cache(false)
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
                    .refresh_cache(false)
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
                    .refresh_cache(false)
                    .no_headers(true)
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        let mut buf = Vec::new();
        validate_and_list(remote, args, &mut buf).unwrap();
        assert_eq!(
            "v0.0.1 | namespace/project:v0.0.1 | registry.gitlab.com/namespace/project:v0.0.1\n",
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
                    .refresh_cache(false)
                    .no_headers(true)
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        let mut buf = Vec::new();
        validate_and_list(remote, args, &mut buf).unwrap();
        assert_eq!(
            "1 | registry.gitlab.com/namespace/project | 10 | 2021-01-01T00:00:00Z\n",
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
                    .refresh_cache(false)
                    .num_pages(true)
                    .no_headers(true)
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
                    .refresh_cache(false)
                    .num_pages(true)
                    .no_headers(true)
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
            .refresh_cache(false)
            .no_headers(false)
            .build()
            .unwrap();
        let mut buf = Vec::new();
        get_image_metadata(remote, args, &mut buf).unwrap();
        assert_eq!(
            "Name | Location | Short SHA | Size | Created at\n\
            v0.0.1 | registry.gitlab.com/namespace/project:v0.0.1 | 12345678 | 100 | 2021-01-01T00:00:00Z\n",
            String::from_utf8(buf).unwrap()
        );
    }

    #[test]
    fn test_get_image_metadata_no_headers() {
        let remote = Arc::new(MockContainerRegistry::new());
        let args = DockerImageCliArgs::builder()
            .tag("v0.0.1".to_string())
            .repo_id(1)
            .refresh_cache(false)
            .no_headers(true)
            .build()
            .unwrap();
        let mut buf = Vec::new();
        get_image_metadata(remote, args, &mut buf).unwrap();
        assert_eq!(
            "v0.0.1 | registry.gitlab.com/namespace/project:v0.0.1 | 12345678 | 100 | 2021-01-01T00:00:00Z\n",
            String::from_utf8(buf).unwrap()
        );
    }
}
