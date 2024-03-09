use crate::{
    api_traits::Deploy,
    cmds::release::{Release, ReleaseBodyArgs},
    io::{HttpRunner, Response},
    Result,
};

use super::Github;

impl<R: HttpRunner<Response = Response>> Deploy for Github<R> {
    fn list(&self, args: ReleaseBodyArgs) -> Result<Vec<Release>> {
        todo!();
    }

    fn num_pages(&self) -> Result<Option<u32>> {
        todo!();
    }
}
