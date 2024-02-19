use crate::api_traits::{ApiOperation, RemoteProject};
use crate::cli::BrowseOptions;
use crate::http::{self};
use crate::io::{CmdInfo, HttpRunner, Response};
use crate::remote::query::{self, gitlab_list_members};
use crate::remote::{Member, Project};
use crate::Result;

use super::Gitlab;

impl<R: HttpRunner<Response = Response>> RemoteProject for Gitlab<R> {
    fn get_project_data(&self, id: Option<i64>) -> Result<CmdInfo> {
        let url = match id {
            Some(id) => format!("{}/{}", self.base_project_url, id),
            None => self.rest_api_basepath().to_string(),
        };
        let project = query::gitlab_project_data::<_, ()>(
            &self.runner,
            &url,
            None,
            self.headers(),
            http::Method::GET,
            ApiOperation::Project,
        )?;
        Ok(CmdInfo::Project(project))
    }

    fn get_project_members(&self) -> Result<CmdInfo> {
        let url = format!("{}/members/all", self.rest_api_basepath());
        let members = gitlab_list_members(
            &self.runner,
            &url,
            None,
            self.headers(),
            None,
            ApiOperation::Project,
        )?;
        Ok(CmdInfo::Members(members))
    }

    fn get_url(&self, option: BrowseOptions) -> String {
        let base_url = format!("https://{}/{}", self.domain, self.path);
        match option {
            BrowseOptions::Repo => base_url,
            BrowseOptions::MergeRequests => format!("{}/merge_requests", base_url),
            BrowseOptions::MergeRequestId(id) => format!("{}/merge_requests/{}", base_url, id),
            BrowseOptions::Pipelines => format!("{}/pipelines", base_url),
        }
    }
}

pub struct GitlabProjectFields {
    id: i64,
    default_branch: String,
    web_url: String,
}

impl From<&serde_json::Value> for GitlabProjectFields {
    fn from(data: &serde_json::Value) -> Self {
        GitlabProjectFields {
            id: data["id"].as_i64().unwrap(),
            default_branch: data["default_branch"].as_str().unwrap().to_string(),
            web_url: data["web_url"].as_str().unwrap().to_string(),
        }
    }
}

impl From<GitlabProjectFields> for Project {
    fn from(fields: GitlabProjectFields) -> Self {
        Project::new(fields.id, &fields.default_branch).with_html_url(&fields.web_url)
    }
}

pub struct GitlabMemberFields {
    id: i64,
    name: String,
    username: String,
    created_at: String,
}

impl From<&serde_json::Value> for GitlabMemberFields {
    fn from(data: &serde_json::Value) -> Self {
        GitlabMemberFields {
            id: data["id"].as_i64().unwrap(),
            name: data["name"].as_str().unwrap().to_string(),
            username: data["username"].as_str().unwrap().to_string(),
            created_at: data["created_at"].as_str().unwrap().to_string(),
        }
    }
}

impl From<GitlabMemberFields> for Member {
    fn from(fields: GitlabMemberFields) -> Self {
        Member::builder()
            .id(fields.id)
            .name(fields.name.to_string())
            .username(fields.username.to_string())
            .created_at(fields.created_at.to_string())
            .build()
            .unwrap()
    }
}

#[cfg(test)]
mod test {
    use std::sync::Arc;

    use crate::api_traits::ApiOperation;
    use crate::test::utils::{config, get_contract, ContractType, MockRunner};

    use crate::io::CmdInfo;

    use super::*;

    #[test]
    fn test_get_project_data_no_id() {
        let config = config();
        let domain = "gitlab.com";
        let path = "jordilin/gitlapi";
        let response = Response::builder()
            .status(200)
            .body(get_contract(ContractType::Gitlab, "project.json"))
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let gitlab = Gitlab::new(config, &domain, &path, client.clone());
        gitlab.get_project_data(None).unwrap();
        assert_eq!(
            "https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi",
            client.url().to_string(),
        );
        assert_eq!("1234", client.headers().get("PRIVATE-TOKEN").unwrap());
        assert_eq!(Some(ApiOperation::Project), *client.api_operation.borrow());
    }

    #[test]
    fn test_get_project_data_with_given_id() {
        let config = config();
        let domain = "gitlab.com";
        let path = "jordilin/gitlapi";
        let response = Response::builder()
            .status(200)
            .body(get_contract(ContractType::Gitlab, "project.json"))
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let gitlab = Gitlab::new(config, &domain, &path, client.clone());
        gitlab.get_project_data(Some(54345)).unwrap();
        assert_eq!(
            "https://gitlab.com/api/v4/projects/54345",
            client.url().to_string(),
        );
        assert_eq!("1234", client.headers().get("PRIVATE-TOKEN").unwrap());
        assert_eq!(Some(ApiOperation::Project), *client.api_operation.borrow());
    }

    #[test]
    fn test_get_project_members() {
        let config = config();
        let domain = "gitlab.com";
        let path = "jordilin/gitlapi";
        let response = Response::builder()
            .status(200)
            .body(get_contract(ContractType::Gitlab, "project_members.json"))
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let gitlab = Gitlab::new(config, &domain, &path, client.clone());

        let CmdInfo::Members(members) = gitlab.get_project_members().unwrap() else {
            panic!("Expected members");
        };
        assert_eq!(2, members.len());
        assert_eq!("test_user_0", members[0].username);
        assert_eq!("test_user_1", members[1].username);
        assert_eq!("1234", client.headers().get("PRIVATE-TOKEN").unwrap());
        assert_eq!(
            "https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/members/all",
            *client.url(),
        );
        assert_eq!(Some(ApiOperation::Project), *client.api_operation.borrow());
    }
}
