use std::sync::Arc;

use crate::{cli::my::MyOptions, config::Config, Result};

use super::merge_request;

pub fn execute(
    options: MyOptions,
    config: Arc<Config>,
    domain: String,
    path: String,
) -> Result<()> {
    match options {
        MyOptions::MergeRequest(cli_args) => {
            merge_request::list_merge_requests(domain, path, config, cli_args)
        }
    }
}
