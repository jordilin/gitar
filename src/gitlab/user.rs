use crate::{
    api_traits::{ApiOperation, UserInfo},
    cmds::project::Member,
    http,
    io::{HttpRunner, Response},
    remote::query,
    Result,
};

use super::Gitlab;

impl<R: HttpRunner<Response = Response>> UserInfo for Gitlab<R> {
    fn get_auth_user(&self) -> Result<Member> {
        let user = query::gitlab_auth_user::<_, ()>(
            &self.runner,
            &self.base_current_user_url,
            None,
            self.headers(),
            http::Method::GET,
            ApiOperation::Project,
        )?;
        Ok(user)
    }

    fn get(&self, _username: &str) -> Result<Member> {
        todo!()
    }
}

pub struct GitlabUserFields {
    id: i64,
    username: String,
    name: String,
}

impl From<&serde_json::Value> for GitlabUserFields {
    fn from(data: &serde_json::Value) -> Self {
        GitlabUserFields {
            id: data["id"].as_i64().unwrap(),
            username: data["username"].as_str().unwrap().to_string(),
            name: data["name"].as_str().unwrap().to_string(),
        }
    }
}

impl From<GitlabUserFields> for Member {
    fn from(fields: GitlabUserFields) -> Self {
        Member::builder()
            .id(fields.id)
            .name(fields.name)
            .username(fields.username)
            .build()
            .unwrap()
    }
}

#[cfg(test)]
mod test {
    use crate::{
        api_traits::ApiOperation,
        setup_client,
        test::utils::{default_gitlab, ContractType, ResponseContracts},
    };

    use super::*;

    #[test]
    fn test_get_user_id() {
        let contracts = ResponseContracts::new(ContractType::Gitlab).add_contract(
            200,
            "get_user_info.json",
            None,
        );
        let (client, gitlab) = setup_client!(contracts, default_gitlab(), dyn UserInfo);
        let user = gitlab.get_auth_user().unwrap();
        assert_eq!(123456, user.id);
        assert_eq!("jordilin", user.username);
        assert_eq!("https://gitlab.com/api/v4/user", *client.url(),);
        assert_eq!("1234", client.headers().get("PRIVATE-TOKEN").unwrap());
        assert_eq!(Some(ApiOperation::Project), *client.api_operation.borrow());
    }
}
