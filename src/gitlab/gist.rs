use crate::{
    api_traits::CodeGist,
    cmds::gist::{Gist, GistListBodyArgs},
    io::{HttpResponse, HttpRunner},
};

use super::Gitlab;

impl<R: HttpRunner<Response = HttpResponse>> CodeGist for Gitlab<R> {
    fn list(&self, _args: GistListBodyArgs) -> crate::Result<Vec<Gist>> {
        todo!()
    }

    fn num_pages(&self) -> crate::Result<Option<u32>> {
        todo!()
    }

    fn num_resources(&self) -> crate::Result<Option<crate::api_traits::NumberDeltaErr>> {
        todo!()
    }
}
