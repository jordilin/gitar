use crate::{
    api_traits::{ApiOperation, Deploy, DeployAsset, NumberDeltaErr},
    cmds::release::{Release, ReleaseAssetListBodyArgs, ReleaseAssetMetadata, ReleaseBodyArgs},
    http,
    io::{HttpResponse, HttpRunner},
    remote::query,
    Result,
};

use super::Gitlab;

impl<R: HttpRunner<Response = HttpResponse>> Deploy for Gitlab<R> {
    fn list(&self, args: ReleaseBodyArgs) -> Result<Vec<Release>> {
        let url = format!("{}/releases", self.rest_api_basepath());
        query::paged(
            &self.runner,
            &url,
            args.from_to_page,
            self.headers(),
            None,
            ApiOperation::Release,
            |value| GitlabReleaseFields::from(value).into(),
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

impl<R: HttpRunner<Response = HttpResponse>> Gitlab<R> {
    fn resource_release_metadata_url(&self) -> (String, http::Headers) {
        let url = format!("{}/releases?page=1", self.rest_api_basepath());
        let headers = self.headers();
        (url, headers)
    }

    fn get_release(&self, args: ReleaseAssetListBodyArgs) -> Result<serde_json::Value> {
        let url = format!("{}/releases/{}", self.rest_api_basepath(), args.id);
        query::get_json::<_, ()>(
            &self.runner,
            &url,
            None,
            self.headers(),
            ApiOperation::Release,
        )
    }
}

impl<R: HttpRunner<Response = HttpResponse>> DeployAsset for Gitlab<R> {
    fn list(&self, args: ReleaseAssetListBodyArgs) -> Result<Vec<ReleaseAssetMetadata>> {
        let release = self.get_release(args)?;
        let mut asset_metadata = Vec::new();
        build_release_assets(&release, &mut asset_metadata, AssetType::Sources);
        // _links is considered an asset in the Gitlab API, include those too.
        build_release_assets(&release, &mut asset_metadata, AssetType::Links);
        Ok(asset_metadata)
    }

    fn num_pages(&self, args: ReleaseAssetListBodyArgs) -> Result<Option<u32>> {
        let url = format!("{}/releases/{}?page=1", self.rest_api_basepath(), args.id);
        // Assets is a one single request to the release API endpoint for
        // Gitlab, so there's only one page available. If the HEAD request
        // succeeds, then set it to one.
        query::num_pages(&self.runner, &url, self.headers(), ApiOperation::Release)?;
        Ok(Some(1))
    }

    fn num_resources(&self, args: ReleaseAssetListBodyArgs) -> Result<Option<NumberDeltaErr>> {
        // Number of resources comes by doing a GET request to the release API
        // See JSON doc contracts/gitlab/list_release_assets.json where the
        // number or resources is in the field assets.count
        let release = self.get_release(args)?;
        let count = release["assets"]["count"].as_u64().unwrap();
        Ok(Some(NumberDeltaErr::new(1, count as u32)))
    }
}

enum AssetType {
    Sources,
    Links,
}

impl AsRef<str> for AssetType {
    fn as_ref(&self) -> &str {
        match self {
            AssetType::Sources => "sources",
            AssetType::Links => "links",
        }
    }
}

fn build_release_assets(
    release: &serde_json::Value,
    asset_metadata: &mut Vec<ReleaseAssetMetadata>,
    asset_type: AssetType,
) {
    let assets = release["assets"][asset_type.as_ref()].as_array().unwrap();
    for asset in assets {
        let asset_data = ReleaseAssetMetadata::builder()
            // There's no id available in the response per se. Grab the short commit
            // id instead
            .id(release["commit"]["short_id"].as_str().unwrap().to_string())
            .name(release["name"].as_str().unwrap().to_string())
            .url(asset["url"].as_str().unwrap().to_string())
            .size("".to_string())
            .created_at(release["created_at"].as_str().unwrap().to_string())
            .updated_at(release["released_at"].as_str().unwrap().to_string())
            .build()
            .unwrap();
        asset_metadata.push(asset_data);
    }
}

pub struct GitlabReleaseFields {
    release: Release,
}

impl From<&serde_json::Value> for GitlabReleaseFields {
    fn from(value: &serde_json::Value) -> Self {
        Self {
            release: Release::builder()
                // There's no id available in the response per se. Grab the short commit
                // id instead
                .id(value["commit"]["short_id"].as_str().unwrap().to_string())
                .url(value["_links"]["self"].as_str().unwrap().to_string())
                .tag(value["tag_name"].as_str().unwrap().to_string())
                .title(value["name"].as_str().unwrap().to_string())
                .description(value["description"].as_str().unwrap().to_string())
                .prerelease(value["upcoming_release"].as_bool().unwrap())
                .created_at(value["created_at"].as_str().unwrap().to_string())
                .updated_at(value["released_at"].as_str().unwrap().to_string())
                .build()
                .unwrap(),
        }
    }
}

impl From<GitlabReleaseFields> for Release {
    fn from(fields: GitlabReleaseFields) -> Self {
        fields.release
    }
}

#[cfg(test)]
mod test {

    use crate::{
        http::Headers,
        setup_client,
        test::utils::{default_gitlab, ContractType, ResponseContracts},
    };

    use super::*;

    #[test]
    fn test_list_release() {
        let contracts = ResponseContracts::new(ContractType::Gitlab).add_contract(
            200,
            "list_releases.json",
            None,
        );
        let (client, gitlab) = setup_client!(contracts, default_gitlab(), dyn Deploy);
        let args = ReleaseBodyArgs::builder()
            .from_to_page(None)
            .build()
            .unwrap();
        let releases = gitlab.list(args).unwrap();
        assert_eq!(
            "https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/releases",
            *client.url(),
        );
        assert_eq!(Some(ApiOperation::Release), *client.api_operation.borrow());
        assert_eq!(1, releases.len());
    }

    #[test]
    fn test_release_num_pages() {
        let link_header = "<https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/releases?page=1>; rel=\"first\", <https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/releases?page=1>; rel=\"last\"";
        let mut headers = Headers::new();
        headers.set("link", link_header);
        let contracts = ResponseContracts::new(ContractType::Gitlab).add_body::<String>(
            200,
            None,
            Some(headers),
        );
        let (client, gitlab) = setup_client!(contracts, default_gitlab(), dyn Deploy);
        let num_pages = gitlab.num_pages().unwrap();
        assert_eq!(
            "https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/releases?page=1",
            *client.url(),
        );
        assert_eq!(Some(ApiOperation::Release), *client.api_operation.borrow());
        assert_eq!(Some(1), num_pages);
    }

    #[test]
    fn test_list_release_assets() {
        let contracts = ResponseContracts::new(ContractType::Gitlab).add_contract(
            200,
            "list_release_assets.json",
            None,
        );
        let (client, gitlab) = setup_client!(contracts, default_gitlab(), dyn DeployAsset);
        let args = ReleaseAssetListBodyArgs::builder()
            .id("v0.1.18-alpha-2".to_string())
            .list_args(None)
            .build()
            .unwrap();
        let assets = gitlab.list(args).unwrap();
        assert_eq!(
            "https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/releases/v0.1.18-alpha-2",
            *client.url(),
        );
        assert_eq!(Some(ApiOperation::Release), *client.api_operation.borrow());
        assert_eq!(5, assets.len());
    }

    #[test]
    fn test_list_release_assets_not_ok_status_code_error() {
        let contracts = ResponseContracts::new(ContractType::Gitlab).add_contract(
            404,
            "list_release_assets.json",
            None,
        );
        let (_, gitlab) = setup_client!(contracts, default_gitlab(), dyn DeployAsset);
        let args = ReleaseAssetListBodyArgs::builder()
            .id("v0.1.18-alpha-2".to_string())
            .list_args(None)
            .build()
            .unwrap();
        let assets = gitlab.list(args);
        assert!(assets.is_err());
    }

    #[test]
    fn test_list_release_assets_num_pages() {
        let contracts = ResponseContracts::new(ContractType::Gitlab).add_contract(
            200,
            "list_release_assets.json",
            None,
        );
        let (client, gitlab) = setup_client!(contracts, default_gitlab(), dyn DeployAsset);
        let args = ReleaseAssetListBodyArgs::builder()
            .id("v0.1.18-alpha-2".to_string())
            .list_args(None)
            .build()
            .unwrap();
        let num_pages = gitlab.num_pages(args).unwrap();
        assert_eq!(
            "https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/releases/v0.1.18-alpha-2?page=1",
            *client.url(),
        );
        assert_eq!(Some(ApiOperation::Release), *client.api_operation.borrow());
        assert_eq!(Some(1), num_pages);
    }

    #[test]
    fn test_list_release_assets_num_resources() {
        let contracts = ResponseContracts::new(ContractType::Gitlab).add_contract(
            200,
            "list_release_assets.json",
            None,
        );
        let (_, gitlab) = setup_client!(contracts, default_gitlab(), dyn DeployAsset);
        let args = ReleaseAssetListBodyArgs::builder()
            .id("v0.1.18-alpha-2".to_string())
            .list_args(None)
            .build()
            .unwrap();
        let num_resources = gitlab.num_resources(args).unwrap().unwrap();
        assert_eq!("(1, 5)", &num_resources.to_string());
    }
}
