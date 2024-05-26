use std::io::Write;
use std::sync::Arc;

use crate::api_traits::{Deploy, DeployAsset, Timestamp};
use crate::cli::release::{ReleaseAssetOptions, ReleaseOptions};
use crate::cmds::common::num_release_pages;
use crate::config::Config;
use crate::display::{Column, DisplayBody};
use crate::remote::{self, ListBodyArgs, ListRemoteCliArgs};
use crate::Result;

use super::common::{
    self, num_release_asset_pages, num_release_asset_resources, num_release_resources,
};

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
    #[builder(default)]
    prerelease: bool,
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
            Column::new("ID", release.id),
            Column::new("Tag", release.tag),
            Column::new("Title", release.title),
            Column::new("Description", release.description),
            Column::new("URL", release.url),
            Column::new("Prerelease", release.prerelease.to_string()),
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

#[derive(Builder, Clone)]
pub struct ReleaseAssetListBodyArgs {
    // It can be a release tag (Gitlab) or an actual release id (Github)
    pub id: String,
    pub list_args: Option<ListBodyArgs>,
}

impl ReleaseAssetListBodyArgs {
    pub fn builder() -> ReleaseAssetListBodyArgsBuilder {
        ReleaseAssetListBodyArgsBuilder::default()
    }
}

#[derive(Builder, Clone)]
pub struct ReleaseAssetMetadata {
    id: String,
    name: String,
    url: String,
    size: String,
    created_at: String,
    updated_at: String,
}

impl ReleaseAssetMetadata {
    pub fn builder() -> ReleaseAssetMetadataBuilder {
        ReleaseAssetMetadataBuilder::default()
    }
}

impl From<ReleaseAssetMetadata> for DisplayBody {
    fn from(asset: ReleaseAssetMetadata) -> Self {
        DisplayBody::new(vec![
            Column::new("ID", asset.id),
            Column::new("Name", asset.name),
            Column::new("URL", asset.url),
            Column::new("Size", asset.size),
            Column::new("Created At", asset.created_at),
            Column::new("Updated At", asset.updated_at),
        ])
    }
}

impl Timestamp for ReleaseAssetMetadata {
    fn created_at(&self) -> String {
        self.created_at.clone()
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
            let from_to_args = remote::validate_from_to_page(&cli_args)?;
            let body_args = ReleaseBodyArgs::builder()
                .from_to_page(from_to_args)
                .build()?;
            list_releases(remote, body_args, cli_args, std::io::stdout())
        }
        ReleaseOptions::Assets(cli_opts) => match cli_opts {
            ReleaseAssetOptions::List(cli_args) => {
                let remote = crate::remote::get_deploy_asset(
                    domain,
                    path,
                    config,
                    cli_args.list_args.get_args.refresh_cache,
                )?;

                let list_args = remote::validate_from_to_page(&cli_args.list_args)?;
                let body_args = ReleaseAssetListBodyArgs::builder()
                    .id(cli_args.id.clone())
                    .list_args(list_args)
                    .build()?;
                if cli_args.list_args.num_pages {
                    return num_release_asset_pages(remote, body_args, std::io::stdout());
                }
                if cli_args.list_args.num_resources {
                    return num_release_asset_resources(remote, body_args, std::io::stdout());
                }
                list_release_assets(remote, body_args, cli_args, std::io::stdout())
            }
        },
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

fn list_release_assets<W: Write>(
    remote: Arc<dyn DeployAsset>,
    body_args: ReleaseAssetListBodyArgs,
    cli_args: ReleaseAssetListCliArgs,
    mut writer: W,
) -> Result<()> {
    common::list_release_assets(remote, body_args, cli_args, &mut writer)
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
            Ok(vec![Release::builder()
                .id(String::from("1"))
                .url(String::from(
                    "https://github.com/jordilin/githapi/releases/tag/v0.1.20",
                ))
                .tag(String::from("v1.0.0"))
                .title(String::from("First release"))
                .description(String::from("Initial release"))
                .created_at(String::from("2021-01-01T00:00:00Z"))
                .updated_at(String::from("2021-01-01T00:00:01Z"))
                .build()
                .unwrap()])
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
            "ID|Tag|Title|Description|URL|Prerelease|Created At|Updated At\n1|v1.0.0|First release|Initial release|https://github.com/jordilin/githapi/releases/tag/v0.1.20|false|2021-01-01T00:00:00Z|2021-01-01T00:00:01Z\n",
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

    impl DeployAsset for MockDeploy {
        fn list(&self, _args: ReleaseAssetListBodyArgs) -> Result<Vec<ReleaseAssetMetadata>> {
            let asset = ReleaseAssetMetadata::builder().
                id("155582366".to_string()).
                name("gr-x86_64-unknown-linux-musl.tar.gz".to_string()).
                url("https://github.com/jordilin/gitar/releases/download/v0.1.28/gr-x86_64-unknown-linux-musl.tar.gz".to_string()).
                size("2871690".to_string())
                .created_at("2024-03-08T08:29:47Z".to_string())
                .updated_at("2024-03-08T08:29:47Z".to_string()).build().unwrap();
            Ok(vec![asset])
        }

        fn num_pages(&self, _args: ReleaseAssetListBodyArgs) -> Result<Option<u32>> {
            todo!()
        }

        fn num_resources(&self, _args: ReleaseAssetListBodyArgs) -> Result<Option<NumberDeltaErr>> {
            todo!()
        }
    }

    #[test]
    fn test_list_release_assets() {
        let remote = Arc::new(MockDeploy::new(false));
        let id = "155582366".to_string();
        let body_args = ReleaseAssetListBodyArgs::builder()
            .id(id.clone())
            .list_args(Some(ListBodyArgs::builder().build().unwrap()))
            .build()
            .unwrap();
        let cli_args = ReleaseAssetListCliArgs::builder()
            .id(id)
            .list_args(ListRemoteCliArgs::builder().build().unwrap())
            .build()
            .unwrap();
        let mut writer = Vec::new();
        list_release_assets(remote, body_args, cli_args, &mut writer).unwrap();
        assert_eq!(
            "ID|Name|URL|Size|Created At|Updated At\n155582366|gr-x86_64-unknown-linux-musl.tar.gz|https://github.com/jordilin/gitar/releases/download/v0.1.28/gr-x86_64-unknown-linux-musl.tar.gz|2871690|2024-03-08T08:29:47Z|2024-03-08T08:29:47Z\n", String::from_utf8(writer).unwrap());
    }
}
