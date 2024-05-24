use std::io::Write;
use std::sync::Arc;

use crate::api_traits::{Deploy, Timestamp};
use crate::cli::release::ReleaseOptions;
use crate::cmds::common::num_release_pages;
use crate::config::Config;
use crate::display::{Column, DisplayBody};
use crate::remote::{ListBodyArgs, ListRemoteCliArgs};
use crate::Result;

use super::common::{self, num_release_resources};

#[derive(Builder, Clone)]
pub struct ReleaseBodyArgs {
    pub from_to_page: Option<ListBodyArgs>,
}

impl ReleaseBodyArgs {
    pub fn builder() -> ReleaseBodyArgsBuilder {
        ReleaseBodyArgsBuilder::default()
    }
}

#[derive(Builder, Clone)]
pub struct Release {
    id: String,
    url: String,
    tag: String,
    title: String,
    description: String,
    created_at: String,
    updated_at: String,
}

impl Release {
    pub fn builder() -> ReleaseBuilder {
        ReleaseBuilder::default()
    }
}

impl From<Release> for DisplayBody {
    fn from(release: Release) -> Self {
        DisplayBody::new(vec![
            Column::new("Tag", release.tag),
            Column::new("Title", release.title),
            Column::new("Description", release.description),
            Column::new("URL", release.url),
            Column::new("ID", release.id),
            Column::new("Created At", release.created_at),
            Column::new("Updated At", release.updated_at),
        ])
    }
}

impl Timestamp for Release {
    fn created_at(&self) -> String {
        self.created_at.clone()
    }
}

#[derive(Builder, Clone)]
pub struct ReleaseAssetListCliArgs {
    pub id: String,
    pub list_args: ListRemoteCliArgs,
}

impl ReleaseAssetListCliArgs {
    pub fn builder() -> ReleaseAssetListCliArgsBuilder {
        ReleaseAssetListCliArgsBuilder::default()
    }
}

pub fn execute(
    options: ReleaseOptions,
    config: Arc<Config>,
    domain: String,
    path: String,
) -> Result<()> {
    match options {
        ReleaseOptions::List(cli_args) => {
            let remote =
                crate::remote::get_deploy(domain, path, config, cli_args.get_args.refresh_cache)?;
            if cli_args.num_pages {
                return num_release_pages(remote, std::io::stdout());
            }
            if cli_args.num_resources {
                return num_release_resources(remote, std::io::stdout());
            }
            let from_to_args = crate::remote::validate_from_to_page(&cli_args)?;
            let body_args = ReleaseBodyArgs::builder()
                .from_to_page(from_to_args)
                .build()?;
            list_releases(remote, body_args, cli_args, std::io::stdout())
        }
        ReleaseOptions::Assets(cli_args) => {
            todo!();
        }
    }
}

fn list_releases<W: Write>(
    remote: Arc<dyn Deploy>,
    body_args: ReleaseBodyArgs,
    cli_args: ListRemoteCliArgs,
    mut writer: W,
) -> Result<()> {
    common::list_releases(remote, body_args, cli_args, &mut writer)
}

#[cfg(test)]
mod test {
    use crate::api_traits::NumberDeltaErr;

    use super::*;

    struct MockDeploy {
        empty_releases: bool,
    }

    impl MockDeploy {
        fn new(empty_releases: bool) -> Self {
            Self { empty_releases }
        }
    }

    impl Deploy for MockDeploy {
        fn list(&self, _args: ReleaseBodyArgs) -> Result<Vec<Release>> {
            if self.empty_releases {
                return Ok(vec![]);
            }
            Ok(vec![Release {
                id: String::from("1"),
                url: String::from("https://github.com/jordilin/githapi/releases/tag/v0.1.20"),
                tag: String::from("v1.0.0"),
                title: String::from("First release"),
                description: String::from("Initial release"),
                created_at: String::from("2021-01-01T00:00:00Z"),
                updated_at: String::from("2021-01-01T00:00:01Z"),
            }])
        }

        fn num_pages(&self) -> Result<Option<u32>> {
            todo!()
        }

        fn num_resources(&self) -> Result<Option<NumberDeltaErr>> {
            todo!()
        }
    }

    #[test]
    fn test_list_releases() {
        let remote = Arc::new(MockDeploy::new(false));
        let body_args = ReleaseBodyArgs::builder()
            .from_to_page(None)
            .build()
            .unwrap();
        let cli_args = ListRemoteCliArgs::builder().build().unwrap();
        let mut writer = Vec::new();
        list_releases(remote, body_args, cli_args, &mut writer).unwrap();
        assert_eq!(
            "Tag|Title|Description|URL|ID|Created At|Updated At\nv1.0.0|First release|Initial release|https://github.com/jordilin/githapi/releases/tag/v0.1.20|1|2021-01-01T00:00:00Z|2021-01-01T00:00:01Z\n",
            String::from_utf8(writer).unwrap(),
        );
    }

    #[test]
    fn test_no_releases_found() {
        let remote = Arc::new(MockDeploy::new(true));
        let body_args = ReleaseBodyArgs::builder()
            .from_to_page(None)
            .build()
            .unwrap();
        let cli_args = ListRemoteCliArgs::builder().build().unwrap();
        let mut writer = Vec::new();
        list_releases(remote, body_args, cli_args, &mut writer).unwrap();
        assert_eq!("No resources found.\n", String::from_utf8(writer).unwrap());
    }

    #[test]
    fn test_list_releases_empty_with_flush_no_warn_message() {
        let remote = Arc::new(MockDeploy::new(true));
        let body_args = ReleaseBodyArgs::builder()
            .from_to_page(None)
            .build()
            .unwrap();
        let cli_args = ListRemoteCliArgs::builder().flush(true).build().unwrap();
        let mut writer = Vec::new();
        list_releases(remote, body_args, cli_args, &mut writer).unwrap();
        assert_eq!("", String::from_utf8(writer).unwrap());
    }
}
