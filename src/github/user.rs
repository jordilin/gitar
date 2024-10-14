use super::Github;
use crate::api_traits::{ApiOperation, UserInfo};
use crate::cmds::project::Member;
use crate::cmds::user::UserCliArgs;
use crate::io::{HttpRunner, HttpResponse};
use crate::remote::query;
use crate::Result;

impl<R: HttpRunner<Response = HttpResponse>> UserInfo for Github<R> {
    fn get_auth_user(&self) -> Result<Member> {
        let url = format!("{}/user", self.rest_api_basepath);
        let user = query::get::<_, (), Member>(
            &self.runner,
            &url,
            None,
            self.request_headers(),
            ApiOperation::Project,
            |value| GithubUserFields::from(value).into(),
        )?;
        Ok(user)
    }

    fn get(&self, _args: &UserCliArgs) -> Result<Member> {
        // https://docs.github.com/en/rest/users/users?apiVersion=2022-11-28#get-a-user
        let url = format!("{}/users/{}", self.rest_api_basepath, _args.username);
        let user = query::get::<_, (), Member>(
            &self.runner,
            &url,
            None,
            self.request_headers(),
            ApiOperation::Project,
            |value| GithubUserFields::from(value).into(),
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

    use crate::{
        api_traits::ApiOperation,
        remote, setup_client,
        test::utils::{default_github, ContractType, ResponseContracts},
    };

    use super::*;

    #[test]
    fn test_get_user_id() {
        let contracts = ResponseContracts::new(ContractType::Github).add_contract(
            200,
            "get_auth_user.json",
            None,
        );
        let (client, github) = setup_client!(contracts, default_github(), dyn UserInfo);
        let user = github.get_auth_user().unwrap();

        assert_eq!(123456, user.id);
        assert_eq!("jdoe", user.username);
        assert_eq!("https://api.github.com/user", *client.url(),);
        assert_eq!(Some(ApiOperation::Project), *client.api_operation.borrow());
    }

    #[test]
    fn test_get_user_by_username() {
        let contracts = ResponseContracts::new(ContractType::Github).add_contract(
            200,
            "get_user_by_username.json",
            None,
        );
        let (client, github) = setup_client!(contracts, default_github(), dyn UserInfo);
        let args = UserCliArgs::builder()
            .username("octocat".to_string())
            .get_args(remote::GetRemoteCliArgs::builder().build().unwrap())
            .build()
            .unwrap();
        let user = github.get(&args).unwrap();

        assert_eq!(1, user.id);
        assert_eq!("octocat", user.username);
        assert_eq!("https://api.github.com/users/octocat", *client.url(),);
        assert_eq!(Some(ApiOperation::Project), *client.api_operation.borrow());
    }
}
