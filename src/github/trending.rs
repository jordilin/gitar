use crate::{
    api_traits::TrendingProjectURL,
    cmds::trending::TrendingProject,
    io::{HttpRunner, Response},
    Result,
};

use super::Github;

impl<R: HttpRunner<Response = Response>> TrendingProjectURL for Github<R> {
    fn list(&self, language: String) -> Result<Vec<TrendingProject>> {
        todo!()
    }
}
