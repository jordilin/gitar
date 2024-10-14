use crate::{
    api_traits::{ApiOperation, UserInfo},
    cmds::{project::Member, user::UserCliArgs},
    error::GRError,
    io::{HttpRunner, HttpResponse},
    remote::{self, query},
    Result,
};

use super::Gitlab;

impl<R: HttpRunner<Response = HttpResponse>> UserInfo for Gitlab<R> {
    fn get_auth_user(&self) -> Result<Member> {
        let user = query::get::<_, (), _>(
            &self.runner,
            &self.base_current_user_url,
            None,
            self.headers(),
            ApiOperation::Project,
            |value| GitlabUserFields::from(value).into(),
        )?;
        Ok(user)
    }

    fn get(&self, args: &UserCliArgs) -> Result<Member> {
        // https://docs.gitlab.com/ee/api/users.html#list-users
        // In Gitlab, getting a user by username is done by using the list users
        // API.
        let url = format!("{}?username={}", self.base_users_url, args.username);
        // Because we are getting a single user, we can limit the number of
        // pages to just 1.
        let list_args = remote::ListBodyArgs::builder()
            .max_pages(1)
            .get_args(args.get_args.clone())
            .build()
            .unwrap();
        let user = query::paged::<_, Member>(
            &self.runner,
            &url,
            Some(list_args),
            self.headers(),
            None,
            ApiOperation::Project,
            |value| GitlabUserFields::from(value).into(),
        )?;
        if user.is_empty() {
            return Err(GRError::UserNotFound(args.username.clone()).into());
        }
        Ok(user[0].clone())
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
        error, setup_client,
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

    #[test]
    fn test_get_user_by_username_ok() {
        let contracts = ResponseContracts::new(ContractType::Gitlab).add_contract(
            200,
            "get_user_by_username.json",
            None,
        );
        let (client, gitlab) = setup_client!(contracts, default_gitlab(), dyn UserInfo);
        let username = "tomsawyer";
        let args = UserCliArgs::builder()
            .username(username.to_string())
            .get_args(remote::GetRemoteCliArgs::builder().build().unwrap())
            .build()
            .unwrap();
        let user = gitlab.get(&args).unwrap();
        assert_eq!(12345, user.id);
        assert_eq!("tomsawyer", user.username);
        assert_eq!(
            "https://gitlab.com/api/v4/users?username=tomsawyer",
            *client.url(),
        );
        assert_eq!("1234", client.headers().get("PRIVATE-TOKEN").unwrap());
        assert_eq!(Some(ApiOperation::Project), *client.api_operation.borrow());
    }

    #[test]
    fn test_username_not_found_is_error() {
        let contracts = ResponseContracts::new(ContractType::Gitlab).add_body::<String>(
            200,
            Some("[]".to_string()),
            None,
        );
        let (_, gitlab) = setup_client!(contracts, default_gitlab(), dyn UserInfo);
        let username = "notfound";
        let args = UserCliArgs::builder()
            .username(username.to_string())
            .get_args(remote::GetRemoteCliArgs::builder().build().unwrap())
            .build()
            .unwrap();
        let result = gitlab.get(&args);
        match result {
            Err(err) => match err.downcast_ref::<error::GRError>() {
                Some(error::GRError::UserNotFound(_)) => {}
                _ => panic!("Expected user not found error"),
            },
            Ok(_) => panic!("Expected user not found error"),
        }
    }
}
