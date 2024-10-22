use crate::{
    api_traits::{ApiOperation, Deploy, DeployAsset, NumberDeltaErr},
    cmds::release::{Release, ReleaseAssetListBodyArgs, ReleaseAssetMetadata, ReleaseBodyArgs},
    io::{HttpResponse, HttpRunner},
    remote::query,
    Result,
};

use super::Github;

impl<R: HttpRunner<Response = HttpResponse>> Deploy for Github<R> {
    fn list(&self, args: ReleaseBodyArgs) -> Result<Vec<Release>> {
        let url = format!("{}/repos/{}/releases", self.rest_api_basepath, self.path);
        query::paged(
            &self.runner,
            &url,
            args.from_to_page,
            self.request_headers(),
            None,
            ApiOperation::Release,
            |value| GithubReleaseFields::from(value).into(),
        )
    }

    fn num_pages(&self) -> Result<Option<u32>> {
        let (url, headers) = self.resource_release_metadata_url();
        query::num_pages(&self.runner, &url, headers, ApiOperation::Release)
    }

    fn num_resources(&self) -> Result<Option<NumberDeltaErr>> {
        let (url, headers) = self.resource_release_metadata_url();
        query::num_resources(&self.runner, &url, headers, ApiOperation::Release)
    }
}

impl<R> Github<R> {
    fn resource_release_metadata_url(&self) -> (String, crate::http::Headers) {
        let url = format!(
            "{}/repos/{}/releases?page=1",
            self.rest_api_basepath, self.path
        );
        let headers = self.request_headers();
        (url, headers)
    }

    fn resource_release_assets_metadata_url(&self, args: ReleaseAssetListBodyArgs) -> String {
        let url = format!(
            "{}/repos/{}/releases/{}/assets?page=1",
            self.rest_api_basepath, self.path, args.id
        );
        url
    }
}

impl<R: HttpRunner<Response = HttpResponse>> DeployAsset for Github<R> {
    fn list(&self, args: ReleaseAssetListBodyArgs) -> Result<Vec<ReleaseAssetMetadata>> {
        let url = format!(
            "{}/repos/{}/releases/{}/assets",
            self.rest_api_basepath, self.path, args.id
        );
        query::paged(
            &self.runner,
            &url,
            args.list_args,
            self.request_headers(),
            None,
            ApiOperation::Release,
            |value| GithubReleaseAssetFields::from(value).into(),
        )
    }

    fn num_pages(&self, args: ReleaseAssetListBodyArgs) -> Result<Option<u32>> {
        let url = self.resource_release_assets_metadata_url(args);
        query::num_pages(
            &self.runner,
            &url,
            self.request_headers(),
            ApiOperation::Release,
        )
    }

    fn num_resources(&self, args: ReleaseAssetListBodyArgs) -> Result<Option<NumberDeltaErr>> {
        let url = self.resource_release_assets_metadata_url(args);
        query::num_resources(
            &self.runner,
            &url,
            self.request_headers(),
            ApiOperation::Release,
        )
    }
}

pub struct GithubReleaseFields {
    release: Release,
}

impl From<&serde_json::Value> for GithubReleaseFields {
    fn from(value: &serde_json::Value) -> Self {
        Self {
            release: Release::builder()
                .id(value["id"].as_i64().unwrap().to_string())
                .url(value["html_url"].as_str().unwrap().to_string())
                .tag(value["tag_name"].as_str().unwrap().to_string())
                .title(value["name"].as_str().unwrap_or_default().to_string())
                .description(value["body"].as_str().unwrap_or_default().to_string())
                .prerelease(value["prerelease"].as_bool().unwrap_or(false))
                .created_at(value["created_at"].as_str().unwrap().to_string())
                .updated_at(value["published_at"].as_str().unwrap().to_string())
                .build()
                .unwrap(),
        }
    }
}

impl From<GithubReleaseFields> for Release {
    fn from(fields: GithubReleaseFields) -> Self {
        fields.release
    }
}

pub struct GithubReleaseAssetFields {
    release_asset: ReleaseAssetMetadata,
}

impl From<&serde_json::Value> for GithubReleaseAssetFields {
    fn from(value: &serde_json::Value) -> Self {
        Self {
            release_asset: ReleaseAssetMetadata::builder()
                .id(value["id"].as_i64().unwrap().to_string())
                .name(value["name"].as_str().unwrap().to_string())
                .url(value["browser_download_url"].as_str().unwrap().to_string())
                .size(value["size"].as_i64().unwrap().to_string())
                .created_at(value["created_at"].as_str().unwrap().to_string())
                .updated_at(value["updated_at"].as_str().unwrap().to_string())
                .build()
                .unwrap(),
        }
    }
}

impl From<GithubReleaseAssetFields> for ReleaseAssetMetadata {
    fn from(fields: GithubReleaseAssetFields) -> Self {
        fields.release_asset
    }
}

#[cfg(test)]
mod test {

    use crate::{
        api_traits::ApiOperation,
        http::Headers,
        setup_client,
        test::utils::{default_github, get_contract, ContractType, ResponseContracts},
    };

    use super::*;

    #[test]
    fn test_list_releases() {
        let contracts = ResponseContracts::new(ContractType::Github).add_contract(
            200,
            "list_releases.json",
            None,
        );
        let (client, github) = setup_client!(contracts, default_github(), dyn Deploy);
        let args = ReleaseBodyArgs::builder()
            .from_to_page(None)
            .build()
            .unwrap();
        let runs = github.list(args).unwrap();
        assert_eq!(
            "https://api.github.com/repos/jordilin/githapi/releases",
            *client.url(),
        );
        assert_eq!(Some(ApiOperation::Release), *client.api_operation.borrow());
        assert_eq!(1, runs.len());
    }

    #[test]
    fn test_release_num_pages() {
        let link_header = "<https://api.github.com/repos/jordilin/githapi/releases?page=2>; rel=\"next\", <https://api.github.com/repos/jordilin/githapi/releases?page=2>; rel=\"last\"";
        let mut headers = Headers::new();
        headers.set("link".to_string(), link_header.to_string());
        let contracts = ResponseContracts::new(ContractType::Github).add_body::<String>(
            200,
            None,
            Some(headers),
        );
        let (client, github) = setup_client!(contracts, default_github(), dyn Deploy);
        let runs = github.num_pages().unwrap();
        assert_eq!(
            "https://api.github.com/repos/jordilin/githapi/releases?page=1",
            *client.url(),
        );
        assert_eq!(Some(ApiOperation::Release), *client.api_operation.borrow());
        assert_eq!(Some(2), runs);
    }

    #[test]
    fn test_list_release_assets() {
        let contracts = ResponseContracts::new(ContractType::Github).add_body(
            200,
            Some(format!(
                "[{}]",
                get_contract(ContractType::Github, "release_asset.json")
            )),
            None,
        );
        let (client, github) = setup_client!(contracts, default_github(), dyn DeployAsset);
        let args = ReleaseAssetListBodyArgs::builder()
            .id("123".to_string())
            .list_args(None)
            .build()
            .unwrap();
        let runs = github.list(args).unwrap();
        assert_eq!(
            "https://api.github.com/repos/jordilin/githapi/releases/123/assets",
            *client.url(),
        );
        assert_eq!(Some(ApiOperation::Release), *client.api_operation.borrow());
        assert_eq!(1, runs.len());
    }

    #[test]
    fn test_query_num_release_assets_pages() {
        let link_header = "<https://api.github.com/repos/jordilin/githapi/releases/123/assets?page=2>; rel=\"next\", <https://api.github.com/repos/jordilin/githapi/releases/123/assets?page=2>; rel=\"last\"";
        let mut headers = Headers::new();
        headers.set("link".to_string(), link_header.to_string());
        let contracts = ResponseContracts::new(ContractType::Github).add_body::<String>(
            200,
            None,
            Some(headers),
        );
        let (client, github) = setup_client!(contracts, default_github(), dyn DeployAsset);
        let args = ReleaseAssetListBodyArgs::builder()
            .id("123".to_string())
            .list_args(None)
            .build()
            .unwrap();
        let runs = github.num_pages(args).unwrap();
        assert_eq!(
            "https://api.github.com/repos/jordilin/githapi/releases/123/assets?page=1",
            *client.url(),
        );
        assert_eq!(Some(ApiOperation::Release), *client.api_operation.borrow());
        assert_eq!(Some(2), runs);
    }

    #[test]
    fn test_query_num_release_assets_resources() {
        let contracts = ResponseContracts::new(ContractType::Github).add_body(
            200,
            Some(format!(
                "[{}]",
                get_contract(ContractType::Github, "release_asset.json")
            )),
            None,
        );
        let (client, github) = setup_client!(contracts, default_github(), dyn DeployAsset);
        let args = ReleaseAssetListBodyArgs::builder()
            .id("123".to_string())
            .list_args(None)
            .build()
            .unwrap();
        github.num_resources(args).unwrap();
        assert_eq!(
            "https://api.github.com/repos/jordilin/githapi/releases/123/assets?page=1",
            *client.url(),
        );
        assert_eq!(Some(ApiOperation::Release), *client.api_operation.borrow());
    }
}
