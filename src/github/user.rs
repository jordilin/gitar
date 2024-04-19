use super::Github;
use crate::api_traits::{ApiOperation, UserInfo};
use crate::io::{HttpRunner, Response};
use crate::remote::{query, Member};
use crate::{http, Result};

impl<R: HttpRunner<Response = Response>> UserInfo for Github<R> {
    fn get(&self) -> Result<Member> {
        let url = format!("{}/user", self.rest_api_basepath);
        let user = query::github_auth_user::<_, ()>(
            &self.runner,
            &url,
            None,
            self.request_headers(),
            http::Method::GET,
            ApiOperation::Project,
        )?;
        Ok(user)
    }
}

pub struct GithubUserFields {
    id: i64,
    login: String,
    name: String,
}

impl From<&serde_json::Value> for GithubUserFields {
    fn from(data: &serde_json::Value) -> Self {
        GithubUserFields {
            id: data["id"].as_i64().unwrap(),
            login: data["login"].as_str().unwrap().to_string(),
            name: data["name"].as_str().unwrap_or_default().to_string(),
        }
    }
}

impl From<GithubUserFields> for Member {
    fn from(fields: GithubUserFields) -> Self {
        Member::builder()
            .id(fields.id)
            .name(fields.name)
            .username(fields.login)
            .build()
            .unwrap()
    }
}

#[cfg(test)]
mod test {
    use std::sync::Arc;

    use crate::{
        api_traits::ApiOperation,
        test::utils::{config, get_contract, ContractType, MockRunner},
    };

    use super::*;

    #[test]
    fn test_get_user_id() {
        let config = config();
        let domain = "github.com".to_string();
        let path = "jordilin/githapi".to_string();
        let response = Response::builder()
            .status(200)
            .body(get_contract(ContractType::Github, "get_user_info.json"))
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let github: Box<dyn UserInfo> =
            Box::new(Github::new(config, &domain, &path, client.clone()));
        let user = github.get().unwrap();

        assert_eq!(123456, user.id);
        assert_eq!("jdoe", user.username);
        assert_eq!("https://api.github.com/user", *client.url(),);
        assert_eq!(Some(ApiOperation::Project), *client.api_operation.borrow());
    }
}
