use crate::{
    api_traits::{ApiOperation, RemoteProject},
    cli::browse::BrowseOptions,
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

    fn list(&self, args: crate::cmds::project::ProjectListBodyArgs) -> Result<Vec<Project>> {
        let url = if args.stars {
            format!("{}/user/starred", self.rest_api_basepath)
        } else {
            let username = args.user.unwrap().username;
            // TODO - not needed - just /user/repos would do
            // See: https://docs.github.com/en/rest/repos/repos?apiVersion=2022-11-28#list-repositories-for-the-authenticated-user
            format!("{}/users/{}/repos", self.rest_api_basepath, username)
        };

        let projects = query::github_list_projects(
            &self.runner,
            &url,
            args.from_to_page,
            self.request_headers(),
            None,
            ApiOperation::Project,
        )?;
        Ok(projects)
    }
}

pub struct GithubProjectFields {
    id: i64,
    default_branch: String,
    html_url: String,
    created_at: String,
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
            created_at: project_data["created_at"]
                .to_string()
                .trim_matches('"')
                .to_string(),
        }
    }
}

impl From<GithubProjectFields> for Project {
    fn from(fields: GithubProjectFields) -> Self {
        Project::new(fields.id, &fields.default_branch)
            .with_html_url(&fields.html_url)
            .with_created_at(&fields.created_at)
    }
}

pub struct GithubMemberFields {
    id: i64,
    login: String,
    name: String,
    created_at: String,
}

impl From<&serde_json::Value> for GithubMemberFields {
    fn from(member_data: &serde_json::Value) -> Self {
        GithubMemberFields {
            id: member_data["id"].as_i64().unwrap(),
            login: member_data["login"].as_str().unwrap().to_string(),
            name: "".to_string(),
            // Github does not provide created_at field in the response for
            // Members (aka contributors). Set it to UNIX epoch.
            created_at: "1970-01-01T00:00:00Z".to_string(),
        }
    }
}

impl From<GithubMemberFields> for Member {
    fn from(fields: GithubMemberFields) -> Self {
        Member::builder()
            .id(fields.id)
            .username(fields.login)
            .name(fields.name)
            .created_at(fields.created_at)
            .build()
            .unwrap()
    }
}

#[cfg(test)]
mod test {
    use std::sync::Arc;

    use crate::{
        cmds::project::ProjectListBodyArgs,
        test::utils::{config, get_contract, ContractType, MockRunner},
    };

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

    #[test]
    fn test_list_current_user_projects() {
        let config = config();
        let domain = "github.com".to_string();
        let path = "jordilin/githapi";
        let projects = format!("[{}]", get_contract(ContractType::Github, "project.json"));
        let response = Response::builder()
            .status(200)
            .body(projects)
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let github = Github::new(config, &domain, &path, client.clone());
        let body_args = ProjectListBodyArgs::builder()
            .from_to_page(None)
            .user(Some(
                Member::builder()
                    .id(1)
                    .name("jdoe".to_string())
                    .username("jdoe".to_string())
                    .build()
                    .unwrap(),
            ))
            .build()
            .unwrap();
        let projects = github.list(body_args).unwrap();
        assert_eq!(1, projects.len());
        assert_eq!("https://api.github.com/users/jdoe/repos", *client.url());
        assert_eq!(Some(ApiOperation::Project), *client.api_operation.borrow());
    }

    #[test]
    fn test_get_my_starred_projects() {
        let config = config();
        let domain = "github.com".to_string();
        let path = "jordilin/githapi";
        let projects = format!("{}", get_contract(ContractType::Github, "stars.json"));
        let response = Response::builder()
            .status(200)
            .body(projects)
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let github = Github::new(config, &domain, &path, client.clone());
        let body_args = ProjectListBodyArgs::builder()
            .from_to_page(None)
            .user(Some(
                Member::builder()
                    .id(1)
                    .name("jdoe".to_string())
                    .username("jdoe".to_string())
                    .build()
                    .unwrap(),
            ))
            .stars(true)
            .build()
            .unwrap();
        let projects = github.list(body_args).unwrap();
        assert_eq!(1, projects.len());
        assert_eq!("https://api.github.com/user/starred", *client.url());
        assert_eq!(Some(ApiOperation::Project), *client.api_operation.borrow());
    }
}
