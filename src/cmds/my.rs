use std::sync::Arc;

use crate::{cli::my::MyOptions, config::Config, remote, Result};

use super::merge_request;

pub fn execute(
    options: MyOptions,
    config: Arc<Config>,
    domain: String,
    path: String,
) -> Result<()> {
    match options {
        MyOptions::MergeRequest(cli_args) => {
            let remote = remote::get_auth_user(
                domain.clone(),
                path.clone(),
                config.clone(),
                cli_args.list_args.refresh_cache,
            )?;
            let user = remote.get()?;
            merge_request::list_merge_requests(domain, path, config, cli_args, Some(user.id))
        }
    }
}

pub struct User {
    pub id: i64,
}

impl User {
    pub fn new(id: i64) -> Self {
        User { id }
    }
}
