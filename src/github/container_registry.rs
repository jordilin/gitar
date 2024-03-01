use crate::{
    api_traits::ContainerRegistry,
    docker::{DockerListBodyArgs, RegistryRepository},
    io::{HttpRunner, Response},
    Result,
};

use super::Github;

impl<R: HttpRunner<Response = Response>> ContainerRegistry for Github<R> {
    fn list_repositories(&self, _args: DockerListBodyArgs) -> Result<Vec<RegistryRepository>> {
        todo!("list_repositories")
    }

    fn list_repository_tags(
        &self,
        _args: DockerListBodyArgs,
    ) -> Result<Vec<crate::docker::RepositoryTag>> {
        todo!()
    }
}
