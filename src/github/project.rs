use crate::{
    api_traits::{ApiOperation, RemoteProject},
    cli::browse::BrowseOptions,
    cmds::project::ProjectListBodyArgs,
    error::GRError,
    http::Method::GET,
    io::{CmdInfo, HttpRunner, Response},
    remote::{
        query::{self, github_list_members},
        Member, Project, URLQueryParamBuilder,
    },
};

use super::Github;
use crate::Result;

impl<R: HttpRunner<Response = Response>> RemoteProject for Github<R> {
    fn get_project_data(&self, id: Option<i64>, path: Option<&str>) -> Result<CmdInfo> {
        // NOTE: What I call project in here is understood as repository in
        // Github parlance. In Github there is also the concept of having
        // projects in a given repository. Getting a repository by ID is not
        // supported in their REST API.
        if let Some(id) = id {
            return Err(GRError::OperationNotSupported(format!(
                "Getting project data by id is not supported in Github: {}",
                id
            ))
            .into());
        };
        let url = if let Some(path) = path {
            format!("{}/repos/{}", self.rest_api_basepath, path)
        } else {
            format!("{}/repos/{}", self.rest_api_basepath, self.path)
        };
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
        let url = self.list_project_url(&args, false);
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

    fn num_pages(&self, args: ProjectListBodyArgs) -> Result<Option<u32>> {
        let url = self.list_project_url(&args, true);
        query::num_pages(
            &self.runner,
            &url,
            self.request_headers(),
            ApiOperation::Project,
        )
    }
}

impl<R> Github<R> {
    fn list_project_url(&self, args: &ProjectListBodyArgs, num_pages: bool) -> String {
        let url = if args.stars {
            format!("{}/user/starred", self.rest_api_basepath)
        } else {
            let username = args.user.as_ref().unwrap().clone().username;
            // TODO - not needed - just /user/repos would do
            // See: https://docs.github.com/en/rest/repos/repos?apiVersion=2022-11-28#list-repositories-for-the-authenticated-user
            format!("{}/users/{}/repos", self.rest_api_basepath, username)
        };
        if num_pages {
            return URLQueryParamBuilder::new(&url)
                .add_param("page", "1")
                .build();
        }
        url
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
    member: Member,
}

impl From<&serde_json::Value> for GithubMemberFields {
    fn from(member_data: &serde_json::Value) -> Self {
        GithubMemberFields {
            member: Member::builder()
                .id(member_data["id"].as_i64().unwrap())
                .username(member_data["login"].as_str().unwrap().to_string())
                .name("".to_string())
                // Github does not provide created_at field in the response for
                // Members (aka contributors). Set it to UNIX epoch.
                .created_at("1970-01-01T00:00:00Z".to_string())
                .build()
                .unwrap(),
        }
    }
}

impl From<GithubMemberFields> for Member {
    fn from(fields: GithubMemberFields) -> Self {
        fields.member
    }
}

#[cfg(test)]
mod test {

    use crate::{
        cmds::project::ProjectListBodyArgs,
        http::Headers,
        setup_client,
        test::utils::{default_github, get_contract, ContractType, ResponseContracts},
    };

    use super::*;

    #[test]
    fn test_get_project_data_no_id() {
        let contracts =
            ResponseContracts::new(ContractType::Github).add_contract(200, "project.json", None);
        let (client, github) = setup_client!(contracts, default_github(), dyn RemoteProject);
        github.get_project_data(None, None).unwrap();
        assert_eq!(
            "https://api.github.com/repos/jordilin/githapi",
            *client.url(),
        );
        assert_eq!(Some(ApiOperation::Project), *client.api_operation.borrow());
    }

    #[test]
    fn test_get_project_data_given_owner_repo_path() {
        let contracts =
            ResponseContracts::new(ContractType::Github).add_contract(200, "project.json", None);
        let (client, github) = setup_client!(contracts, default_github(), dyn RemoteProject);
        let result = github.get_project_data(None, Some("jordilin/gitar"));
        assert_eq!("https://api.github.com/repos/jordilin/gitar", *client.url(),);
        match result {
            Ok(CmdInfo::Project(project)) => {
                assert_eq!(123456, project.id);
            }
            _ => panic!("Expected project data"),
        }
    }

    #[test]
    fn test_get_project_data_with_id_not_supported() {
        let contracts = ResponseContracts::new(ContractType::Github);
        let (_, github) = setup_client!(contracts, default_github(), dyn RemoteProject);
        assert!(github.get_project_data(Some(1), None).is_err());
    }

    #[test]
    fn test_list_current_user_projects() {
        let contracts = ResponseContracts::new(ContractType::Github).add_body(
            200,
            Some(format!(
                "[{}]",
                get_contract(ContractType::Github, "project.json")
            )),
            None,
        );
        let (client, github) = setup_client!(contracts, default_github(), dyn RemoteProject);
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
        let contracts =
            ResponseContracts::new(ContractType::Github).add_contract(200, "stars.json", None);
        let (client, github) = setup_client!(contracts, default_github(), dyn RemoteProject);
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

    #[test]
    fn test_get_project_num_pages_url_for_user() {
        let link_header = "<https://api.github.com/users/jdoe/repos?page=2>; rel=\"next\", <https://api.github.com/users/jdoe/repos?page=2>; rel=\"last\"";
        let mut headers = Headers::new();
        headers.set("link", link_header);
        let contracts = ResponseContracts::new(ContractType::Github).add_body::<String>(
            200,
            None,
            Some(headers),
        );
        let (client, github) = setup_client!(contracts, default_github(), dyn RemoteProject);
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
        github.num_pages(body_args).unwrap();
        assert_eq!(
            "https://api.github.com/users/jdoe/repos?page=1",
            *client.url()
        );
        assert_eq!(Some(ApiOperation::Project), *client.api_operation.borrow());
    }

    #[test]
    fn test_get_project_num_pages_url_for_starred() {
        let link_header = "<https://api.github.com/user/starred?page=2>; rel=\"next\", <https://api.github.com/user/starred?page=2>; rel=\"last\"";
        let mut headers = Headers::new();
        headers.set("link", link_header);
        let contracts = ResponseContracts::new(ContractType::Github).add_body::<String>(
            200,
            None,
            Some(headers),
        );
        let (client, github) = setup_client!(contracts, default_github(), dyn RemoteProject);
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
        github.num_pages(body_args).unwrap();
        assert_eq!("https://api.github.com/user/starred?page=1", *client.url());
        assert_eq!(Some(ApiOperation::Project), *client.api_operation.borrow());
    }
}
