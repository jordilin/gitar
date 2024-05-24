use crate::{
    api_traits::{ApiOperation, Deploy, DeployAsset, NumberDeltaErr},
    cmds::release::{Release, ReleaseAssetListBodyArgs, ReleaseAssetMetadata, ReleaseBodyArgs},
    io::{HttpRunner, Response},
    remote::query,
    Result,
};

use super::Github;

impl<R: HttpRunner<Response = Response>> Deploy for Github<R> {
    fn list(&self, args: ReleaseBodyArgs) -> Result<Vec<Release>> {
        let url = format!("{}/repos/{}/releases", self.rest_api_basepath, self.path);
        query::github_releases(
            &self.runner,
            &url,
            args.from_to_page,
            self.request_headers(),
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

impl<R> Github<R> {
    fn resource_release_metadata_url(&self) -> (String, crate::http::Headers) {
        let url = format!(
            "{}/repos/{}/releases?page=1",
            self.rest_api_basepath, self.path
        );
        let headers = self.request_headers();
        (url, headers)
    }
}

impl<R: HttpRunner<Response = Response>> DeployAsset for Github<R> {
    fn list(&self, args: ReleaseAssetListBodyArgs) -> Result<Vec<ReleaseAssetMetadata>> {
        todo!()
    }

    fn num_pages(&self, args: ReleaseAssetListBodyArgs) -> Result<Option<u32>> {
        todo!()
    }

    fn num_resources(&self, args: ReleaseAssetListBodyArgs) -> Result<Option<NumberDeltaErr>> {
        todo!()
    }
}

pub struct GithubReleaseFields {
    id: String,
    url: String,
    tag: String,
    title: String,
    description: String,
    created_at: String,
    updated_at: String,
}

impl From<&serde_json::Value> for GithubReleaseFields {
    fn from(value: &serde_json::Value) -> Self {
        Self {
            id: value["id"].as_i64().unwrap().to_string(),
            url: value["html_url"].as_str().unwrap().to_string(),
            tag: value["tag_name"].as_str().unwrap().to_string(),
            title: value["name"].as_str().unwrap().to_string(),
            description: value["body"].as_str().unwrap_or_default().to_string(),
            created_at: value["created_at"].as_str().unwrap().to_string(),
            updated_at: value["published_at"].as_str().unwrap().to_string(),
        }
    }
}

impl From<GithubReleaseFields> for Release {
    fn from(fields: GithubReleaseFields) -> Self {
        Release::builder()
            .id(fields.id)
            .url(fields.url)
            .tag(fields.tag)
            .title(fields.title)
            .description(fields.description)
            .created_at(fields.created_at)
            .updated_at(fields.updated_at)
            .build()
            .unwrap()
    }
}

#[cfg(test)]
mod test {

    use crate::{
        api_traits::ApiOperation,
        http::Headers,
        setup_client,
        test::utils::{default_github, ContractType, ResponseContracts},
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
}
