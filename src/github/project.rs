use crate::{
    api_traits::{ApiOperation, RemoteProject},
    cli::BrowseOptions,
    error::GRError,
    http::Method::GET,
    io::{CmdInfo, HttpRunner, Response},
    remote::{
        query::{self, github_list_members},
        Member, Project,
    },
};

use super::Github;
use crate::Result;

impl<R: HttpRunner<Response = Response>> RemoteProject for Github<R> {
    fn get_project_data(&self, id: Option<i64>) -> Result<CmdInfo> {
        if let Some(id) = id {
            return Err(GRError::OperationNotSupported(format!(
                "Getting project data by id is not supported in Github: {}",
                id
            ))
            .into());
        };
        let url = format!("{}/repos/{}", self.rest_api_basepath, self.path);
        let project = query::github_project_data::<_, ()>(
            &self.runner,
            &url,
            None,
            self.request_headers(),
            GET,
            ApiOperation::Project,
        )?;
        Ok(CmdInfo::Project(project))
    }

    fn get_project_members(&self) -> Result<CmdInfo> {
        let url = &format!(
            "{}/repos/{}/contributors",
            self.rest_api_basepath, self.path
        );
        let members = github_list_members(
            &self.runner,
            url,
            None,
            self.request_headers(),
            None,
            ApiOperation::Project,
        )?;
        Ok(CmdInfo::Members(members))
    }

    fn get_url(&self, option: BrowseOptions) -> String {
        let base_url = format!("https://{}/{}", self.domain, self.path);
        match option {
            BrowseOptions::Repo => base_url,
            BrowseOptions::MergeRequests => format!("{}/pulls", base_url),
            BrowseOptions::MergeRequestId(id) => format!("{}/pull/{}", base_url, id),
            BrowseOptions::Pipelines => format!("{}/actions", base_url),
        }
    }
}

pub struct GithubProjectFields {
    id: i64,
    default_branch: String,
    html_url: String,
}

impl From<&serde_json::Value> for GithubProjectFields {
    fn from(project_data: &serde_json::Value) -> Self {
        GithubProjectFields {
            id: project_data["id"].as_i64().unwrap(),
            default_branch: project_data["default_branch"]
                .to_string()
                .trim_matches('"')
                .to_string(),
            html_url: project_data["html_url"]
                .to_string()
                .trim_matches('"')
                .to_string(),
        }
    }
}

impl From<GithubProjectFields> for Project {
    fn from(fields: GithubProjectFields) -> Self {
        Project::new(fields.id, &fields.default_branch).with_html_url(&fields.html_url)
    }
}

pub struct GithubMemberFields {
    id: i64,
    login: String,
    name: String,
}

impl From<&serde_json::Value> for GithubMemberFields {
    fn from(member_data: &serde_json::Value) -> Self {
        GithubMemberFields {
            id: member_data["id"].as_i64().unwrap(),
            login: member_data["login"].as_str().unwrap().to_string(),
            name: "".to_string(),
        }
    }
}

impl From<GithubMemberFields> for Member {
    fn from(fields: GithubMemberFields) -> Self {
        Member::builder()
            .id(fields.id)
            .username(fields.login)
            .name(fields.name)
            .build()
            .unwrap()
    }
}

#[cfg(test)]
mod test {
    use std::sync::Arc;

    use crate::test::utils::{config, get_contract, ContractType, MockRunner};

    use super::*;

    #[test]
    fn test_get_project_data_no_id() {
        let config = config();
        let domain = "github.com".to_string();
        let path = "jordilin/githapi";
        let response = Response::builder()
            .status(200)
            .body(get_contract(ContractType::Github, "project.json"))
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let github = Github::new(config, &domain, &path, client.clone());
        github.get_project_data(None).unwrap();
        assert_eq!(
            "https://api.github.com/repos/jordilin/githapi",
            *client.url(),
        );
        assert_eq!(Some(ApiOperation::Project), *client.api_operation.borrow());
    }

    #[test]
    fn test_get_project_data_with_id_not_supported() {
        let config = config();
        let domain = "github.com".to_string();
        let path = "jordilin/githapi";
        let client = Arc::new(MockRunner::new(vec![]));
        let github = Github::new(config, &domain, &path, client.clone());
        assert!(github.get_project_data(Some(1)).is_err());
    }
}
