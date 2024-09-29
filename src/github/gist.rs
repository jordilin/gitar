use crate::{
    api_traits::{ApiOperation, CodeGist, NumberDeltaErr},
    cmds::gist::{Gist, GistListBodyArgs},
    io::{HttpRunner, Response},
    remote::{query, URLQueryParamBuilder},
    Result,
};

use super::Github;

// https://docs.github.com/en/rest/gists/gists?apiVersion=2022-11-28

impl<R: HttpRunner<Response = Response>> CodeGist for Github<R> {
    fn list(&self, args: GistListBodyArgs) -> crate::Result<Vec<Gist>> {
        let url = self.auth_user_gist_url(false);
        query::paged(
            &self.runner,
            &url,
            args.body_args,
            self.request_headers(),
            None,
            ApiOperation::Gist,
            |value| GithubGistFields::from(value).into(),
        )
    }

    fn num_pages(&self) -> Result<Option<u32>> {
        let url = self.auth_user_gist_url(true);
        query::num_pages(
            &self.runner,
            &url,
            self.request_headers(),
            ApiOperation::Gist,
        )
    }

    fn num_resources(&self) -> Result<Option<NumberDeltaErr>> {
        let url = self.auth_user_gist_url(true);
        query::num_resources(
            &self.runner,
            &url,
            self.request_headers(),
            ApiOperation::Gist,
        )
    }
}

impl<R> Github<R> {
    fn auth_user_gist_url(&self, first_page: bool) -> String {
        let url = format!("{}/gists", self.rest_api_basepath);
        let mut url_query_param = URLQueryParamBuilder::new(&url);
        if first_page {
            url_query_param.add_param("page", "1");
        }
        url_query_param.build()
    }
}

pub struct GithubGistFields {
    pub gist: Gist,
}

impl From<&serde_json::Value> for GithubGistFields {
    fn from(value: &serde_json::Value) -> Self {
        let gist = Gist::builder()
            .url(value["html_url"].as_str().unwrap().to_string())
            .description(value["description"].as_str().unwrap().to_string())
            .files(
                value["files"]
                    .as_object()
                    .unwrap_or(&serde_json::Map::new())
                    .keys()
                    .map(|k| k.to_string())
                    .collect::<Vec<String>>()
                    .join(","),
            )
            .created_at(value["created_at"].as_str().unwrap_or("").to_string())
            .build()
            .unwrap();
        Self { gist }
    }
}

impl From<GithubGistFields> for Gist {
    fn from(fields: GithubGistFields) -> Self {
        fields.gist
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        setup_client,
        test::utils::{default_github, ContractType, ResponseContracts},
    };

    use super::*;

    #[test]
    fn test_github_list_user_gists() {
        let contracts = ResponseContracts::new(ContractType::Github).add_contract(
            200,
            "list_user_gist.json",
            None,
        );
        let args = GistListBodyArgs::builder().body_args(None).build().unwrap();
        let (client, github) = setup_client!(contracts, default_github(), dyn CodeGist);
        let gists = github.list(args).unwrap();
        assert_eq!("https://api.github.com/gists", *client.url());
        assert_eq!(1, gists.len());
        assert_eq!(Some(ApiOperation::Gist), *client.api_operation.borrow());
    }

    #[test]
    fn test_github_num_pages() {
        let contracts = ResponseContracts::new(ContractType::Github).add_contract(
            200,
            "list_user_gist.json",
            None,
        );
        let (client, github) = setup_client!(contracts, default_github(), dyn CodeGist);
        let num_pages = github.num_pages().unwrap();
        assert_eq!("https://api.github.com/gists?page=1", *client.url());
        assert_eq!(1, num_pages.unwrap());
        assert_eq!(Some(ApiOperation::Gist), *client.api_operation.borrow());
    }

    #[test]
    fn test_github_num_resources() {
        let contracts = ResponseContracts::new(ContractType::Github).add_contract(
            200,
            "list_user_gist.json",
            None,
        );
        let (client, github) = setup_client!(contracts, default_github(), dyn CodeGist);
        github.num_resources().unwrap();
        assert_eq!("https://api.github.com/gists?page=1", *client.url());
        assert_eq!(Some(ApiOperation::Gist), *client.api_operation.borrow());
    }
}
