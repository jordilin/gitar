use crate::{
    api_traits::{ApiOperation, ProjectMember, RemoteProject, RemoteTag},
    cli::browse::BrowseOptions,
    cmds::project::{Member, Project, ProjectListBodyArgs, Tag},
    error::GRError,
    io::{CmdInfo, HttpResponse, HttpRunner},
    remote::{query, URLQueryParamBuilder},
};

use super::Github;
use crate::Result;

impl<R: HttpRunner<Response = HttpResponse>> RemoteProject for Github<R> {
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
        let project = query::get::<_, (), Project>(
            &self.runner,
            &url,
            None,
            self.request_headers(),
            ApiOperation::Project,
            |value| GithubProjectFields::from(value).into(),
        )?;
        Ok(CmdInfo::Project(project))
    }

    fn get_project_members(&self) -> Result<CmdInfo> {
        let url = &format!(
            "{}/repos/{}/contributors",
            self.rest_api_basepath, self.path
        );
        let members = query::paged(
            &self.runner,
            url,
            None,
            self.request_headers(),
            None,
            ApiOperation::Project,
            |value| GithubMemberFields::from(value).into(),
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
            BrowseOptions::PipelineId(id) => format!("{}/actions/runs/{}", base_url, id),
            BrowseOptions::Releases => format!("{}/releases", base_url),
            // Manual is only one URL and it's the user guide. Handled in the
            // browser command.
            BrowseOptions::Manual => unreachable!(),
        }
    }

    fn list(&self, args: crate::cmds::project::ProjectListBodyArgs) -> Result<Vec<Project>> {
        let url = self.list_project_url(&args, false);
        let projects = query::paged(
            &self.runner,
            &url,
            args.from_to_page,
            self.request_headers(),
            None,
            ApiOperation::Project,
            |value| GithubProjectFields::from(value).into(),
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

    fn num_resources(
        &self,
        args: ProjectListBodyArgs,
    ) -> Result<Option<crate::api_traits::NumberDeltaErr>> {
        let url = self.list_project_url(&args, true);
        query::num_resources(
            &self.runner,
            &url,
            self.request_headers(),
            ApiOperation::Project,
        )
    }
}

impl<R: HttpRunner<Response = HttpResponse>> RemoteTag for Github<R> {
    // https://docs.github.com/en/rest/repos/repos?apiVersion=2022-11-28#list-repository-tags
    fn list(&self, args: ProjectListBodyArgs) -> Result<Vec<Tag>> {
        let url = self.list_project_url(&args, false);
        let tags = query::paged(
            &self.runner,
            &url,
            args.from_to_page,
            self.request_headers(),
            None,
            ApiOperation::RepositoryTag,
            |value| GithubRepositoryTagFields::from(value).into(),
        )?;
        Ok(tags)
    }
}

impl<R: HttpRunner<Response = HttpResponse>> ProjectMember for Github<R> {
    fn list(&self, args: ProjectListBodyArgs) -> Result<Vec<Member>> {
        let url = &format!(
            "{}/repos/{}/contributors",
            self.rest_api_basepath, self.path
        );
        let members = query::paged(
            &self.runner,
            url,
            args.from_to_page,
            self.request_headers(),
            None,
            ApiOperation::Project,
            |value| GithubMemberFields::from(value).into(),
        )?;
        Ok(members)
    }
}

pub struct GithubRepositoryTagFields {
    tags: Tag,
}

impl From<&serde_json::Value> for GithubRepositoryTagFields {
    fn from(tag_data: &serde_json::Value) -> Self {
        GithubRepositoryTagFields {
            tags: Tag::builder()
                .name(tag_data["name"].as_str().unwrap().to_string())
                .sha(tag_data["commit"]["sha"].as_str().unwrap().to_string())
                // Github response does not provide a created_at field, so set
                // it up to UNIX epoch.
                .created_at("1970-01-01T00:00:00Z".to_string())
                .build()
                .unwrap(),
        }
    }
}

impl From<GithubRepositoryTagFields> for Tag {
    fn from(fields: GithubRepositoryTagFields) -> Self {
        fields.tags
    }
}

impl<R> Github<R> {
    fn list_project_url(&self, args: &ProjectListBodyArgs, num_pages: bool) -> String {
        let mut url = if args.tags {
            URLQueryParamBuilder::new(&format!(
                "{}/repos/{}/tags",
                self.rest_api_basepath, self.path
            ))
        } else if args.members {
            URLQueryParamBuilder::new(&format!(
                "{}/repos/{}/contributors",
                self.rest_api_basepath, self.path
            ))
        } else if args.stars {
            URLQueryParamBuilder::new(&format!("{}/user/starred", self.rest_api_basepath))
        } else {
            let username = args.user.as_ref().unwrap().clone().username;
            // TODO - not needed - just /user/repos would do
            // See: https://docs.github.com/en/rest/repos/repos?apiVersion=2022-11-28#list-repositories-for-the-authenticated-user
            URLQueryParamBuilder::new(&format!(
                "{}/users/{}/repos",
                self.rest_api_basepath, username
            ))
        };
        if num_pages {
            return url.add_param("page", "1").build();
        }
        url.build()
    }
}

pub struct GithubProjectFields {
    project: Project,
}

impl From<&serde_json::Value> for GithubProjectFields {
    fn from(project_data: &serde_json::Value) -> Self {
        GithubProjectFields {
            project: Project::builder()
                .id(project_data["id"].as_i64().unwrap())
                .default_branch(project_data["default_branch"].as_str().unwrap().to_string())
                .html_url(project_data["html_url"].as_str().unwrap().to_string())
                .created_at(project_data["created_at"].as_str().unwrap().to_string())
                .description(
                    project_data["description"]
                        .as_str()
                        .unwrap_or_default()
                        .to_string(),
                )
                .language(
                    project_data["language"]
                        .as_str()
                        .unwrap_or_default()
                        .to_string(),
                )
                .build()
                .unwrap(),
        }
    }
}

impl From<GithubProjectFields> for Project {
    fn from(fields: GithubProjectFields) -> Self {
        fields.project
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

    #[test]
    fn test_get_url_pipeline_id() {
        let contracts = ResponseContracts::new(ContractType::Github);
        let (_, github) = setup_client!(contracts, default_github(), dyn RemoteProject);
        let url = github.get_url(BrowseOptions::PipelineId(9527070386));
        assert_eq!(
            "https://github.com/jordilin/githapi/actions/runs/9527070386",
            url
        );
    }

    #[test]
    fn test_list_project_tags() {
        let contracts =
            ResponseContracts::new(ContractType::Github).add_contract(200, "list_tags.json", None);
        let (client, github) = setup_client!(contracts, default_github(), dyn RemoteTag);
        let body_args = ProjectListBodyArgs::builder()
            .user(None)
            .from_to_page(None)
            .tags(true)
            .build()
            .unwrap();
        let tags = RemoteTag::list(&*github, body_args).unwrap();
        assert_eq!(1, tags.len());
        assert_eq!(
            "https://api.github.com/repos/jordilin/githapi/tags",
            *client.url()
        );
        assert_eq!(
            Some(ApiOperation::RepositoryTag),
            *client.api_operation.borrow()
        );
    }

    #[test]
    fn test_get_project_tags_num_pages() {
        let link_header = "<https://api.github.com/repos/jordilin/githapi/tags?page=2>; rel=\"next\", <https://api.github.com/repos/jordilin/githapi/tags?page=2>; rel=\"last\"";
        let mut headers = Headers::new();
        headers.set("link", link_header);
        let contracts = ResponseContracts::new(ContractType::Github).add_body::<String>(
            200,
            None,
            Some(headers),
        );
        let (client, github) = setup_client!(contracts, default_github(), dyn RemoteTag);
        let body_args = ProjectListBodyArgs::builder()
            .user(None)
            .from_to_page(None)
            .tags(true)
            .build()
            .unwrap();
        github.num_pages(body_args).unwrap();
        assert_eq!(
            "https://api.github.com/repos/jordilin/githapi/tags?page=1",
            *client.url()
        );
    }

    #[test]
    fn test_list_project_members() {
        let contracts = ResponseContracts::new(ContractType::Github).add_contract(
            200,
            "project_members.json",
            None,
        );
        let (client, github) = setup_client!(contracts, default_github(), dyn ProjectMember);
        let args = ProjectListBodyArgs::builder()
            .members(true)
            .user(None)
            .from_to_page(None)
            .build()
            .unwrap();
        let members = ProjectMember::list(&*github, args).unwrap();
        assert_eq!(1, members.len());
        assert_eq!("octocat", members[0].username);
        assert_eq!(
            "bearer 1234",
            client.headers().get("Authorization").unwrap()
        );
        assert_eq!(
            "https://api.github.com/repos/jordilin/githapi/contributors",
            *client.url()
        );
        assert_eq!(Some(ApiOperation::Project), *client.api_operation.borrow());
    }

    #[test]
    fn test_project_members_num_pages() {
        let link_header = "<https://api.github.com/repos/jordilin/githapi/contributors?page=2>; rel=\"next\", <https://api.github.com/repos/jordilin/githapi/contributors?page=2>; rel=\"last\"";
        let mut headers = Headers::new();
        headers.set("link", link_header);
        let contracts = ResponseContracts::new(ContractType::Github).add_body::<String>(
            200,
            None,
            Some(headers),
        );
        let (client, github) = setup_client!(contracts, default_github(), dyn ProjectMember);
        let args = ProjectListBodyArgs::builder()
            .members(true)
            .user(None)
            .from_to_page(None)
            .build()
            .unwrap();
        github.num_pages(args).unwrap();
        assert_eq!(
            "https://api.github.com/repos/jordilin/githapi/contributors?page=1",
            *client.url()
        );
    }

    #[test]
    fn test_get_project_members_num_resources() {
        let contracts = ResponseContracts::new(ContractType::Github).add_contract(
            200,
            "project_members.json",
            None,
        );
        let (client, github) = setup_client!(contracts, default_github(), dyn ProjectMember);
        let args = ProjectListBodyArgs::builder()
            .members(true)
            .user(None)
            .from_to_page(None)
            .build()
            .unwrap();
        github.num_resources(args).unwrap();
        assert_eq!(
            "https://api.github.com/repos/jordilin/githapi/contributors?page=1",
            *client.url()
        );
    }
}
