use crate::{
    api_traits::{ApiOperation, Deploy, DeployAsset, NumberDeltaErr},
    cmds::release::{Release, ReleaseAssetListBodyArgs, ReleaseAssetMetadata, ReleaseBodyArgs},
    http::{self, Method::GET},
    io::{HttpRunner, Response},
    json_loads,
    remote::query,
    Result,
};

use super::Gitlab;

impl<R: HttpRunner<Response = Response>> Deploy for Gitlab<R> {
    fn list(&self, args: ReleaseBodyArgs) -> Result<Vec<Release>> {
        let url = format!("{}/releases", self.rest_api_basepath());
        query::gitlab_releases(
            &self.runner,
            &url,
            args.from_to_page,
            self.headers(),
            None,
            ApiOperation::Release,
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

impl<R> Gitlab<R> {
    fn resource_release_metadata_url(&self) -> (String, http::Headers) {
        let url = format!("{}/releases?page=1", self.rest_api_basepath());
        let headers = self.headers();
        (url, headers)
    }
}

impl<R: HttpRunner<Response = Response>> DeployAsset for Gitlab<R> {
    fn list(&self, args: ReleaseAssetListBodyArgs) -> Result<Vec<ReleaseAssetMetadata>> {
        let url = format!("{}/releases/{}", self.rest_api_basepath(), args.id);
        let response = query::gitlab_get_release::<_, ()>(
            &self.runner,
            &url,
            None,
            self.headers(),
            GET,
            ApiOperation::Release,
        )?;
        let mut asset_metatadata = Vec::new();
        let release = json_loads(&response.body)?;
        let assets = release["assets"]["sources"].as_array().unwrap();
        for asset in assets {
            let asset_data = ReleaseAssetMetadata::builder()
                .id(release["commit"]["short_id"].as_str().unwrap().to_string())
                .name(release["name"].as_str().unwrap().to_string())
                .url(asset["url"].as_str().unwrap().to_string())
                .size("".to_string())
                .created_at(release["created_at"].as_str().unwrap().to_string())
                .updated_at(release["released_at"].as_str().unwrap().to_string())
                .build()
                .unwrap();
            asset_metatadata.push(asset_data);
        }
        Ok(asset_metatadata)
    }

    fn num_pages(&self, _args: ReleaseAssetListBodyArgs) -> Result<Option<u32>> {
        todo!()
    }

    fn num_resources(&self, _args: ReleaseAssetListBodyArgs) -> Result<Option<NumberDeltaErr>> {
        todo!()
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
        assert_eq!(4, assets.len());
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
}
