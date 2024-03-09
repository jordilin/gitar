use crate::{
    api_traits::{ApiOperation, Deploy},
    cmds::release::{Release, ReleaseBodyArgs},
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
        let url = format!(
            "{}/repos/{}/releases?page=1",
            self.rest_api_basepath, self.path
        );
        let headers = self.request_headers();
        query::num_pages(&self.runner, &url, headers, ApiOperation::Release)
    }
}

pub struct GithubReleaseFields {
    id: i64,
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
            id: value["id"].as_i64().unwrap(),
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
    use std::sync::Arc;

    use crate::{
        api_traits::ApiOperation,
        http::Headers,
        test::utils::{config, get_contract, ContractType, MockRunner},
    };

    use super::*;

    #[test]
    fn test_list_releases() {
        let config = config();
        let domain = "github.com".to_string();
        let path = "jordilin/githapi";
        let response = Response::builder()
            .status(200)
            .body(get_contract(ContractType::Github, "list_releases.json"))
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let github: Box<dyn Deploy> = Box::new(Github::new(config, &domain, &path, client.clone()));
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
        let config = config();
        let domain = "github.com".to_string();
        let path = "jordilin/githapi";
        let link_header = "<https://api.github.com/repos/jordilin/githapi/releases?page=2>; rel=\"next\", <https://api.github.com/repos/jordilin/githapi/releases?page=2>; rel=\"last\"";
        let mut headers = Headers::new();
        headers.set("link".to_string(), link_header.to_string());
        let response = Response::builder()
            .status(200)
            .headers(headers)
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let github: Box<dyn Deploy> = Box::new(Github::new(config, &domain, &path, client.clone()));
        let runs = github.num_pages().unwrap();
        assert_eq!(
            "https://api.github.com/repos/jordilin/githapi/releases?page=1",
            *client.url(),
        );
        assert_eq!(Some(ApiOperation::Release), *client.api_operation.borrow());
        assert_eq!(Some(2), runs);
    }
}
