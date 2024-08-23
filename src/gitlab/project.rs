use crate::api_traits::{ApiOperation, RemoteProject, RemoteTag};
use crate::cli::browse::BrowseOptions;
use crate::cmds::project::{Member, Project, ProjectListBodyArgs, Tag};
use crate::error::GRError;
use crate::gitlab::encode_path;
use crate::http::{self};
use crate::io::{CmdInfo, HttpRunner, Response};
use crate::remote::query::{self, gitlab_list_members};
use crate::remote::URLQueryParamBuilder;
use crate::Result;

use super::Gitlab;

impl<R: HttpRunner<Response = Response>> RemoteProject for Gitlab<R> {
    fn get_project_data(&self, id: Option<i64>, path: Option<&str>) -> Result<CmdInfo> {
        let url = match (id, path) {
            (Some(id), None) => format!("{}/{}", self.base_project_url, id),
            (None, Some(path)) => {
                format!("{}/{}", self.base_project_url, encode_path(path))
            }
            (None, None) => self.rest_api_basepath().to_string(),
            (Some(_), Some(_)) => {
                return Err(GRError::ApplicationError(
                    "Invalid arguments, can only get project data by id or by owner/repo path"
                        .to_string(),
                )
                .into());
            }
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
            BrowseOptions::MergeRequestId(id) => format!("{}/-/merge_requests/{}", base_url, id),
            BrowseOptions::Pipelines => format!("{}/pipelines", base_url),
            BrowseOptions::PipelineId(id) => format!("{}/-/pipelines/{}", base_url, id),
            BrowseOptions::Releases => format!("{}/-/releases", base_url),
            // Manual is only one URL and it's the user guide. Handled in the
            // browser command.
            BrowseOptions::Manual => unreachable!(),
        }
    }

    fn list(&self, args: ProjectListBodyArgs) -> Result<Vec<Project>> {
        let url = self.list_project_url(&args, false);
        let projects = query::gitlab_list_projects(
            &self.runner,
            &url,
            args.from_to_page,
            self.headers(),
            None,
            ApiOperation::Project,
        )?;
        Ok(projects)
    }

    fn num_pages(&self, args: ProjectListBodyArgs) -> Result<Option<u32>> {
        let url = self.list_project_url(&args, true);
        query::num_pages(&self.runner, &url, self.headers(), ApiOperation::Project)
    }

    fn num_resources(
        &self,
        args: ProjectListBodyArgs,
    ) -> Result<Option<crate::api_traits::NumberDeltaErr>> {
        let url = self.list_project_url(&args, true);
        query::num_resources(&self.runner, &url, self.headers(), ApiOperation::Project)
    }
}

impl<R: HttpRunner<Response = Response>> RemoteTag for Gitlab<R> {
    // https://docs.gitlab.com/ee/api/tags.html
    fn list(&self, args: ProjectListBodyArgs) -> Result<Vec<Tag>> {
        let url = format!("{}/repository/tags", self.projects_base_url);
        let tags = query::gitlab_list_project_tags(
            &self.runner,
            &url,
            args.from_to_page,
            self.headers(),
            None,
            ApiOperation::RepositoryTag,
        )?;
        Ok(tags)
    }
    // NOTE: For num_resources and num_pages, the ApiOperation::Project from the
    // RemoteProject trait is being used, but those operations involve a single
    // HEAD request, which is not cached and does not require pagination. So,
    // technically speaking is not required. Might be a TODO to change/add an
    // ApiOperation to reflect this.
}

impl<R> Gitlab<R> {
    fn list_project_url(&self, args: &ProjectListBodyArgs, num_pages: bool) -> String {
        let mut url = if args.tags {
            URLQueryParamBuilder::new(&format!("{}/repository/tags", self.projects_base_url))
        } else {
            let user = args.user.as_ref().unwrap().clone();
            if args.stars {
                URLQueryParamBuilder::new(&format!(
                    "{}/{}/starred_projects",
                    self.base_users_url, user.id
                ))
            } else {
                URLQueryParamBuilder::new(&format!("{}/{}/projects", self.base_users_url, user.id))
            }
        };
        if num_pages {
            return url.add_param("page", "1").build();
        }
        url.build()
    }
}

pub struct GitlabProjectTagFields {
    tag: Tag,
}

impl From<&serde_json::Value> for GitlabProjectTagFields {
    fn from(data: &serde_json::Value) -> Self {
        GitlabProjectTagFields {
            tag: Tag::builder()
                .name(data["name"].as_str().unwrap().to_string())
                .sha(data["commit"]["id"].as_str().unwrap().to_string())
                .created_at(data["created_at"].as_str().unwrap().to_string())
                .build()
                .unwrap(),
        }
    }
}

impl From<GitlabProjectTagFields> for Tag {
    fn from(fields: GitlabProjectTagFields) -> Self {
        fields.tag
    }
}

pub struct GitlabProjectFields {
    project: Project,
}

impl From<&serde_json::Value> for GitlabProjectFields {
    fn from(data: &serde_json::Value) -> Self {
        GitlabProjectFields {
            project: Project::builder()
                .id(data["id"].as_i64().unwrap())
                .default_branch(data["default_branch"].as_str().unwrap().to_string())
                .html_url(data["web_url"].as_str().unwrap().to_string())
                .created_at(data["created_at"].as_str().unwrap().to_string())
                .description(data["description"].as_str().unwrap_or_default().to_string())
                // NOTE: Project language key is not present in the Gitlab API response.
                .build()
                .unwrap(),
        }
    }
}

impl From<GitlabProjectFields> for Project {
    fn from(fields: GitlabProjectFields) -> Self {
        fields.project
    }
}

pub struct GitlabMemberFields {
    member: Member,
}

impl From<&serde_json::Value> for GitlabMemberFields {
    fn from(data: &serde_json::Value) -> Self {
        GitlabMemberFields {
            member: Member::builder()
                .id(data["id"].as_i64().unwrap())
                .name(data["name"].as_str().unwrap().to_string())
                .username(data["username"].as_str().unwrap().to_string())
                .created_at(data["created_at"].as_str().unwrap().to_string())
                .build()
                .unwrap(),
        }
    }
}

impl From<GitlabMemberFields> for Member {
    fn from(fields: GitlabMemberFields) -> Self {
        fields.member
    }
}

#[cfg(test)]
mod test {

    use crate::api_traits::ApiOperation;
    use crate::cmds::project::ProjectListBodyArgs;
    use crate::http::Headers;
    use crate::setup_client;
    use crate::test::utils::{
        default_gitlab, get_contract, BasePath, ClientType, ContractType, Domain, ResponseContracts,
    };

    use crate::io::CmdInfo;

    use super::*;

    #[test]
    fn test_get_project_data_no_id() {
        let contracts =
            ResponseContracts::new(ContractType::Gitlab).add_contract(200, "project.json", None);
        let (client, gitlab) = setup_client!(contracts, default_gitlab(), dyn RemoteProject);
        gitlab.get_project_data(None, None).unwrap();
        assert_eq!(
            "https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi",
            client.url().to_string(),
        );
        assert_eq!("1234", client.headers().get("PRIVATE-TOKEN").unwrap());
        assert_eq!(Some(ApiOperation::Project), *client.api_operation.borrow());
    }

    #[test]
    fn test_get_project_data_with_given_id() {
        let contracts =
            ResponseContracts::new(ContractType::Gitlab).add_contract(200, "project.json", None);
        let (client, gitlab) = setup_client!(contracts, default_gitlab(), dyn RemoteProject);
        gitlab.get_project_data(Some(54345), None).unwrap();
        assert_eq!(
            "https://gitlab.com/api/v4/projects/54345",
            client.url().to_string(),
        );
        assert_eq!("1234", client.headers().get("PRIVATE-TOKEN").unwrap());
        assert_eq!(Some(ApiOperation::Project), *client.api_operation.borrow());
    }

    #[test]
    fn test_get_project_data_given_owner_repo_path() {
        // current repository path where user is cd'd into.
        let path = "gitlab-org/gitlab-foss";
        let client_type =
            ClientType::Gitlab(Domain("gitlab.com".to_string()), BasePath(path.to_string()));
        let contracts =
            ResponseContracts::new(ContractType::Gitlab).add_contract(200, "project.json", None);
        let (client, gitlab) = setup_client!(contracts, client_type, dyn RemoteProject);
        // User requests information on a different repository.
        let result = gitlab.get_project_data(None, Some("jordilin/gitlapi"));
        assert_eq!(
            "https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi",
            client.url().to_string(),
        );
        match result {
            Ok(CmdInfo::Project(project)) => {
                assert_eq!(44438708, project.id);
            }
            _ => panic!("Expected project"),
        }
    }

    #[test]
    fn test_get_project_data_error_if_both_id_and_path_given() {
        let contracts = ResponseContracts::new(ContractType::Gitlab);
        let (_, gitlab) = setup_client!(contracts, default_gitlab(), dyn RemoteProject);
        let result = gitlab.get_project_data(Some(54345), Some("jordilin/gitlapi"));
        match result {
            Err(err) => match err.downcast_ref::<GRError>() {
                Some(GRError::ApplicationError(msg)) => {
                    assert_eq!(
                        "Invalid arguments, can only get project data by id or by owner/repo path",
                        msg
                    );
                }
                _ => panic!("Expected ApplicationError"),
            },
            _ => panic!("Expected ApplicationError"),
        }
    }

    #[test]
    fn test_get_project_members() {
        let contracts = ResponseContracts::new(ContractType::Gitlab).add_contract(
            200,
            "project_members.json",
            None,
        );
        let (client, gitlab) = setup_client!(contracts, default_gitlab(), dyn RemoteProject);
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

    #[test]
    fn test_list_user_projects() {
        let contracts = ResponseContracts::new(ContractType::Gitlab).add_body(
            200,
            Some(format!(
                "[{}]",
                get_contract(ContractType::Gitlab, "project.json")
            )),
            None,
        );
        let (client, gitlab) = setup_client!(contracts, default_gitlab(), dyn RemoteProject);
        let body_args = ProjectListBodyArgs::builder()
            .from_to_page(None)
            .user(Some(
                Member::builder()
                    .id(1)
                    .name("jordi".to_string())
                    .username("jordilin".to_string())
                    .build()
                    .unwrap(),
            ))
            .build()
            .unwrap();
        gitlab.list(body_args).unwrap();
        assert_eq!(
            "https://gitlab.com/api/v4/users/1/projects",
            client.url().to_string(),
        );
        assert_eq!("1234", client.headers().get("PRIVATE-TOKEN").unwrap());
        assert_eq!(Some(ApiOperation::Project), *client.api_operation.borrow());
    }

    #[test]
    fn test_get_my_starred_projects() {
        let contracts =
            ResponseContracts::new(ContractType::Gitlab).add_contract(200, "stars.json", None);
        let (client, gitlab) = setup_client!(contracts, default_gitlab(), dyn RemoteProject);
        let body_args = ProjectListBodyArgs::builder()
            .from_to_page(None)
            .user(Some(
                Member::builder()
                    .id(1)
                    .name("jordi".to_string())
                    .username("jordilin".to_string())
                    .build()
                    .unwrap(),
            ))
            .stars(true)
            .build()
            .unwrap();
        gitlab.list(body_args).unwrap();
        assert_eq!(
            "https://gitlab.com/api/v4/users/1/starred_projects",
            client.url().to_string(),
        );
        assert_eq!("1234", client.headers().get("PRIVATE-TOKEN").unwrap());
        assert_eq!(Some(ApiOperation::Project), *client.api_operation.borrow());
    }

    #[test]
    fn test_get_num_pages_url_for_user_projects() {
        let link_headers = "<https://gitlab.com/api/v4/users/1/projects?page=2&per_page=20>; rel=\"next\", <https://gitlab.com/api/v4/users/1/projects?page=2&per_page=20>; rel=\"last\"";
        let mut headers = Headers::new();
        headers.set("link", link_headers);
        let contracts = ResponseContracts::new(ContractType::Gitlab).add_contract(
            200,
            "project.json",
            Some(headers),
        );
        let (client, gitlab) = setup_client!(contracts, default_gitlab(), dyn RemoteProject);
        let body_args = ProjectListBodyArgs::builder()
            .from_to_page(None)
            .user(Some(
                Member::builder()
                    .id(1)
                    .name("jordi".to_string())
                    .username("jordilin".to_string())
                    .build()
                    .unwrap(),
            ))
            .build()
            .unwrap();
        gitlab.num_pages(body_args).unwrap();
        assert_eq!(
            "https://gitlab.com/api/v4/users/1/projects?page=1",
            client.url().to_string(),
        );
        assert_eq!(
            ApiOperation::Project,
            *client.api_operation.borrow().as_ref().unwrap()
        );
    }

    #[test]
    fn test_get_project_num_pages_url_for_starred() {
        let link_headers = "<https://gitlab.com/api/v4/users/1/starred_projects?page=2&per_page=20>; rel=\"next\", <https://gitlab.com/api/v4/users/1/starred_projects?page=2&per_page=20>; rel=\"last\"";
        let mut headers = Headers::new();
        headers.set("link", link_headers);
        let contracts = ResponseContracts::new(ContractType::Gitlab).add_contract(
            200,
            "project.json",
            Some(headers),
        );
        let (client, gitlab) = setup_client!(contracts, default_gitlab(), dyn RemoteProject);
        let body_args = ProjectListBodyArgs::builder()
            .from_to_page(None)
            .user(Some(
                Member::builder()
                    .id(1)
                    .name("jordi".to_string())
                    .username("jordilin".to_string())
                    .build()
                    .unwrap(),
            ))
            .stars(true)
            .build()
            .unwrap();
        gitlab.num_pages(body_args).unwrap();
        assert_eq!(
            "https://gitlab.com/api/v4/users/1/starred_projects?page=1",
            client.url().to_string(),
        );
        assert_eq!(
            ApiOperation::Project,
            *client.api_operation.borrow().as_ref().unwrap()
        );
    }

    #[test]
    fn test_get_url_pipeline_id() {
        let contracts = ResponseContracts::new(ContractType::Gitlab);
        let (_, github) = setup_client!(contracts, default_gitlab(), dyn RemoteProject);
        let url = github.get_url(BrowseOptions::PipelineId(9527070386));
        assert_eq!(
            "https://gitlab.com/jordilin/gitlapi/-/pipelines/9527070386",
            url
        );
    }

    #[test]
    fn test_list_tags() {
        let contracts =
            ResponseContracts::new(ContractType::Gitlab).add_contract(200, "list_tags.json", None);
        let (client, gitlab) = setup_client!(contracts, default_gitlab(), dyn RemoteTag);
        let body_args = ProjectListBodyArgs::builder()
            .user(None)
            .from_to_page(None)
            .tags(true)
            .build()
            .unwrap();
        RemoteTag::list(&*gitlab, body_args).unwrap();
        assert_eq!(
            "https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/repository/tags",
            *client.url()
        );
        assert_eq!(
            ApiOperation::RepositoryTag,
            *client.api_operation.borrow().as_ref().unwrap()
        );
    }

    #[test]
    fn test_get_project_tags_num_pages() {
        let link_header = "<https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/repository/tags?page=2&per_page=20>; rel=\"next\", <https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/repository/tags?page=2&per_page=20>; rel=\"last\"";
        let mut headers = Headers::new();
        headers.set("link", link_header);
        let contracts = ResponseContracts::new(ContractType::Gitlab).add_body::<String>(
            200,
            None,
            Some(headers),
        );
        let (client, gitlab) = setup_client!(contracts, default_gitlab(), dyn RemoteTag);
        let body_args = ProjectListBodyArgs::builder()
            .user(None)
            .from_to_page(None)
            .tags(true)
            .build()
            .unwrap();
        gitlab.num_pages(body_args).unwrap();
        assert_eq!(
            "https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/repository/tags?page=1",
            *client.url()
        );
    }
}
