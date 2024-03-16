use crate::{
    api_traits::{ApiOperation, UserInfo},
    cmds::my::User,
    http,
    io::{HttpRunner, Response},
    remote::query,
    Result,
};

use super::Gitlab;

impl<R: HttpRunner<Response = Response>> UserInfo for Gitlab<R> {
    fn get(&self) -> Result<User> {
        let user = query::gitlab_auth_user::<_, ()>(
            &self.runner,
            &self.base_user_url,
            None,
            self.headers(),
            http::Method::GET,
            ApiOperation::Project,
        )?;
        Ok(user)
    }
}

pub struct GitlabUserFields {
    id: i64,
}

impl From<&serde_json::Value> for GitlabUserFields {
    fn from(data: &serde_json::Value) -> Self {
        GitlabUserFields {
            id: data["id"].as_i64().unwrap(),
        }
    }
}

impl From<GitlabUserFields> for User {
    fn from(fields: GitlabUserFields) -> Self {
        User::new(fields.id)
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
        let domain = "gitlab.com".to_string();
        let path = "jordilin/gitlapi".to_string();
        let response = Response::builder()
            .status(200)
            .body(get_contract(ContractType::Gitlab, "get_user_info.json"))
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let gitlab: Box<dyn UserInfo> =
            Box::new(Gitlab::new(config, &domain, &path, client.clone()));
        let user = gitlab.get().unwrap();

        assert_eq!(123456, user.id);
        assert_eq!("https://gitlab.com/api/v4/user", *client.url(),);
        assert_eq!("1234", client.headers().get("PRIVATE-TOKEN").unwrap());
        assert_eq!(Some(ApiOperation::Project), *client.api_operation.borrow());
    }
}
