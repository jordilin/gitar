use crate::{
    api_traits::TrendingProjectURL,
    cmds::trending::TrendingProject,
    io::{HttpRunner, Response},
    Result,
};

use super::Gitlab;

impl<R: HttpRunner<Response = Response>> TrendingProjectURL for Gitlab<R> {
    fn list(&self, _language: String) -> Result<Vec<TrendingProject>> {
        unimplemented!()
    }
}
