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

#[derive(Builder, Clone)]
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

#[derive(Builder, Clone)]
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

impl Display for RepositoryTag {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{} | {} | {}", self.name, self.path, self.location)
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
    }
}

fn validate_and_list<W: Write>(
    remote: Arc<dyn ContainerRegistry + Send + Sync>,
    cli_args: DockerListCliArgs,
    writer: W,
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
        return list_repository_tags(remote, body_args, writer);
    }
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
    let headers = "Name | Path | Location\n";
    writer.write_all(headers.as_bytes())?;
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
    let headers = "ID | Location | Tags count | Created at\n";
    writer.write_all(headers.as_bytes())?;
    for repo in remote.list_repositories(args)? {
        writer.write_all(format!("{}\n", repo).as_bytes())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockContainerRegistry {}

    impl MockContainerRegistry {
        pub fn new() -> Self {
            MockContainerRegistry {}
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
            Ok(Some(1))
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
}
