use std::{
    fmt::{self, Display, Formatter},
    io::Write,
    sync::Arc,
};

use crate::{
    api_traits::ContainerRegistry,
    cli::DockerOptions,
    config::Config,
    remote::{self, get_registry, ListBodyArgs, ListRemoteCliArgs},
    Result,
};

#[derive(Builder, Clone)]
pub struct DockerListCliArgs {
    // If set, list all remote repositories in project's registry
    pub repos: bool,
    pub list_args: ListRemoteCliArgs,
}

impl DockerListCliArgs {
    pub fn builder() -> DockerListCliArgsBuilder {
        DockerListCliArgsBuilder::default()
    }
}

#[derive(Builder, Clone)]

pub struct DockerListBodyArgs {
    pub repos: bool,
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

pub fn execute(
    options: DockerOptions,
    config: Arc<Config>,
    domain: String,
    path: String,
) -> Result<()> {
    match options {
        DockerOptions::List(cli_args) => {
            let remote = get_registry(domain, path, config, cli_args.list_args.refresh_cache)?;
            let body_args = remote::validate_from_to_page(&cli_args.list_args)?;
            let body_args = DockerListBodyArgs::builder()
                .repos(cli_args.repos)
                .body_args(body_args)
                .build()?;
            list_repositories(remote, body_args, std::io::stdout())
        }
    }
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
    }

    #[test]
    fn test_execute_list_repositories() {
        let remote = Arc::new(MockContainerRegistry::new());
        let args = DockerListBodyArgs::builder().repos(true).build().unwrap();
        let mut buf = Vec::new();
        list_repositories(remote, args, &mut buf).unwrap();
        assert_eq!(
            "ID | Location | Tags count | Created at\n\
             1 | registry.gitlab.com/namespace/project | 10 | 2021-01-01T00:00:00Z\n",
            String::from_utf8(buf).unwrap()
        );
    }
}
