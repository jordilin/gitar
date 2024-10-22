use crate::{
    api_traits::TrendingProjectURL,
    cmds::trending::TrendingProject,
    io::{HttpResponse, HttpRunner},
    Result,
};

use super::Gitlab;

impl<R: HttpRunner<Response = HttpResponse>> TrendingProjectURL for Gitlab<R> {
    fn list(&self, _language: String) -> Result<Vec<TrendingProject>> {
        unimplemented!()
    }
}
