use crate::{
    api_traits::{ApiOperation, Deploy},
    cmds::release::{Release, ReleaseBodyArgs},
    io::{HttpRunner, Response},
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
        let url = format!("{}/releases?page=1", self.rest_api_basepath());
        let headers = self.headers();
        query::num_pages(&self.runner, &url, headers, ApiOperation::Release)
    }
}

pub struct GitlabReleaseFields {
    id: String,
    url: String,
    tag: String,
    title: String,
    description: String,
    created_at: String,
    updated_at: String,
}

impl From<&serde_json::Value> for GitlabReleaseFields {
    fn from(value: &serde_json::Value) -> Self {
        Self {
            // There's no id available in the response per se. Grab the short commit
            // id instead
            id: value["commit"]["short_id"].as_str().unwrap().to_string(),
            url: value["_links"]["self"].as_str().unwrap().to_string(),
            tag: value["tag_name"].as_str().unwrap().to_string(),
            title: value["name"].as_str().unwrap().to_string(),
            description: value["description"].as_str().unwrap().to_string(),
            created_at: value["created_at"].as_str().unwrap().to_string(),
            updated_at: value["released_at"].as_str().unwrap().to_string(),
        }
    }
}

impl From<GitlabReleaseFields> for Release {
    fn from(fields: GitlabReleaseFields) -> Self {
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
        http::Headers,
        test::utils::{config, get_contract, ContractType, MockRunner},
    };

    use super::*;

    #[test]
    fn test_list_release() {
        let config = config();
        let domain = "gitlab.com".to_string();
        let path = "jordilin/gitlapi";
        let response = Response::builder()
            .status(200)
            .body(get_contract(ContractType::Gitlab, "list_releases.json"))
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let gitlab: Box<dyn Deploy> = Box::new(Gitlab::new(config, &domain, &path, client.clone()));
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
        let config = config();
        let domain = "gitlab.com".to_string();
        let path = "jordilin/gitlapi";
        let link_header = "<https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/releases?page=1>; rel=\"first\", <https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/releases?page=1>; rel=\"last\"";
        let mut headers = Headers::new();
        headers.set("link", link_header);
        let response = Response::builder()
            .status(200)
            .headers(headers)
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let gitlab: Box<dyn Deploy> = Box::new(Gitlab::new(config, &domain, &path, client.clone()));
        let num_pages = gitlab.num_pages().unwrap();
        assert_eq!(
            "https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/releases?page=1",
            *client.url(),
        );
        assert_eq!(Some(ApiOperation::Release), *client.api_operation.borrow());
        assert_eq!(Some(1), num_pages);
    }
}
