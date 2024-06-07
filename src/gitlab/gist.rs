use crate::{
    api_traits::CodeGist,
    cmds::gist::{Gist, GistListBodyArgs},
    io::{HttpRunner, Response},
};

use super::Gitlab;

impl<R: HttpRunner<Response = Response>> CodeGist for Gitlab<R> {
    fn list(&self, _args: GistListBodyArgs) -> crate::Result<Vec<Gist>> {
        todo!()
    }
}
