use crate::{
    api_traits::ContainerRegistry,
    cmds::docker::{DockerListBodyArgs, ImageMetadata, RegistryRepository, RepositoryTag},
    io::{HttpRunner, Response},
    Result,
};

use super::Github;

impl<R: HttpRunner<Response = Response>> ContainerRegistry for Github<R> {
    fn list_repositories(&self, _args: DockerListBodyArgs) -> Result<Vec<RegistryRepository>> {
        todo!("list_repositories")
    }

    fn list_repository_tags(&self, _args: DockerListBodyArgs) -> Result<Vec<RepositoryTag>> {
        todo!()
    }

    fn num_pages_repository_tags(&self, _repository_id: i64) -> Result<Option<u32>> {
        todo!()
    }

    fn num_pages_repositories(&self) -> Result<Option<u32>> {
        todo!()
    }

    fn get_image_metadata(&self, _repository_id: i64, _tag: &str) -> Result<ImageMetadata> {
        todo!()
    }
}
