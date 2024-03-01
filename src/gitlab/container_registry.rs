use crate::{
    api_traits::{ApiOperation, ContainerRegistry},
    docker::{DockerListBodyArgs, RegistryRepository, RepositoryTag},
    io::{HttpRunner, Response},
    remote::query,
    Result,
};

use super::Gitlab;

impl<R: HttpRunner<Response = Response>> ContainerRegistry for Gitlab<R> {
    fn list_repositories(&self, args: DockerListBodyArgs) -> Result<Vec<RegistryRepository>> {
        let url = format!(
            "{}/registry/repositories?tags_count=true",
            self.rest_api_basepath()
        );
        query::gitlab_project_registry_repositories(
            &self.runner,
            &url,
            args.body_args,
            self.headers(),
            None,
            ApiOperation::ContainerRegistry,
        )
    }

    fn list_repository_tags(
        &self,
        args: DockerListBodyArgs,
    ) -> Result<Vec<crate::docker::RepositoryTag>> {
        // if tags is provided, then args.repo_id is Some at this point. This is
        // enforced at the cli clap level.
        let repository_id = args.repo_id.unwrap();
        let url = format!(
            "{}/registry/repositories/{}/tags",
            self.rest_api_basepath(),
            repository_id
        );
        query::gitlab_project_registry_repository_tags(
            &self.runner,
            &url,
            args.body_args,
            self.headers(),
            None,
            ApiOperation::ContainerRegistry,
        )
    }
}

pub struct GitlabRegistryRepositoryFields {
    id: i64,
    location: String,
    tags_count: i64,
    created_at: String,
}

impl From<&serde_json::Value> for GitlabRegistryRepositoryFields {
    fn from(data: &serde_json::Value) -> Self {
        GitlabRegistryRepositoryFields {
            id: data["id"].as_i64().unwrap(),
            location: data["location"].as_str().unwrap().to_string(),
            tags_count: data["tags_count"].as_i64().unwrap(),
            created_at: data["created_at"].as_str().unwrap().to_string(),
        }
    }
}

impl From<GitlabRegistryRepositoryFields> for RegistryRepository {
    fn from(data: GitlabRegistryRepositoryFields) -> Self {
        RegistryRepository::builder()
            .id(data.id)
            .location(data.location)
            .tags_count(data.tags_count)
            .created_at(data.created_at)
            .build()
            .unwrap()
    }
}

pub struct GitlabRepositoryTagFields {
    name: String,
    path: String,
    location: String,
    created_at: String,
}

impl From<&serde_json::Value> for GitlabRepositoryTagFields {
    fn from(data: &serde_json::Value) -> Self {
        GitlabRepositoryTagFields {
            name: data["name"].as_str().unwrap().to_string(),
            path: data["path"].as_str().unwrap().to_string(),
            location: data["location"].as_str().unwrap().to_string(),
            // Repository tags don't have a creation date. It is included when
            // querying a specific tag. Just return default UNIX epoch date.
            created_at: "1970-01-01T00:00:00Z".to_string(),
        }
    }
}

impl From<GitlabRepositoryTagFields> for RepositoryTag {
    fn from(data: GitlabRepositoryTagFields) -> Self {
        crate::docker::RepositoryTag::builder()
            .name(data.name)
            .path(data.path)
            .location(data.location)
            .created_at(data.created_at)
            .build()
            .unwrap()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::test::utils::{config, get_contract, ContractType, MockRunner};
    use std::sync::Arc;

    #[test]
    fn test_list_repositories_url() {
        let config = config();
        let domain = "gitlab.com";
        let path = "jordilin/gitlapi";
        let response = Response::builder()
            .status(200)
            .body(get_contract(
                ContractType::Gitlab,
                "list_registry_repositories.json",
            ))
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let gitlab = Gitlab::new(config, &domain, &path, client.clone());
        let args = DockerListBodyArgs::builder().repos(true).build().unwrap();
        gitlab.list_repositories(args).unwrap();
        assert_eq!(
            "https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/registry/repositories?tags_count=true",
            client.url().to_string(),
        );
        assert_eq!("1234", client.headers().get("PRIVATE-TOKEN").unwrap());
        assert_eq!(
            Some(ApiOperation::ContainerRegistry),
            *client.api_operation.borrow()
        );
    }

    #[test]
    fn test_list_repository_tags_url() {
        let config = config();
        let domain = "gitlab.com";
        let path = "jordilin/gitlapi";
        let response = Response::builder()
            .status(200)
            .body(get_contract(
                ContractType::Gitlab,
                "list_registry_repository_tags.json",
            ))
            .build()
            .unwrap();
        let client = Arc::new(MockRunner::new(vec![response]));
        let gitlab = Gitlab::new(config, &domain, &path, client.clone());
        let args = DockerListBodyArgs::builder()
            .repos(false)
            .tags(true)
            .repo_id(Some(1))
            .build()
            .unwrap();
        gitlab.list_repository_tags(args).unwrap();
        assert_eq!(
            client.url().to_string(),
            "https://gitlab.com/api/v4/projects/jordilin%2Fgitlapi/registry/repositories/1/tags"
        );
        assert_eq!("1234", client.headers().get("PRIVATE-TOKEN").unwrap());
        assert_eq!(
            Some(ApiOperation::ContainerRegistry),
            *client.api_operation.borrow()
        );
    }
}
