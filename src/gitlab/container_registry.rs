use crate::{
    api_traits::ContainerRegistry,
    docker::{DockerListBodyArgs, RegistryRepository},
    io::{HttpRunner, Response},
    Result,
};

use super::Gitlab;

impl<R: HttpRunner<Response = Response>> ContainerRegistry for Gitlab<R> {
    fn list_repositories(&self, _args: DockerListBodyArgs) -> Result<Vec<RegistryRepository>> {
        todo!("list_repositories")
    }
}
