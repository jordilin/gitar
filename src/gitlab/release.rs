use crate::{
    api_traits::Deploy,
    cmds::release::{Release, ReleaseBodyArgs},
    io::{HttpRunner, Response},
    Result,
};

use super::Gitlab;

impl<R: HttpRunner<Response = Response>> Deploy for Gitlab<R> {
    fn list(&self, _args: ReleaseBodyArgs) -> Result<Vec<Release>> {
        todo!();
    }

    fn num_pages(&self) -> Result<Option<u32>> {
        todo!();
    }
}
