use std::sync::Arc;

use crate::cli::BrowseOptions;
use crate::config::Config;
use crate::remote;
use crate::Result;

pub fn execute(
    options: BrowseOptions,
    config: Arc<Config>,
    domain: String,
    path: String,
) -> Result<()> {
    match options {
        BrowseOptions::Repo => {
            // No need to contact the remote object, domain and path already
            // computed.
            let remote_url = format!("https://{}/{}", domain, path);
            Ok(open::that(remote_url)?)
        }
        BrowseOptions::MergeRequests => {
            let remote = remote::get(domain, path, config, false)?;
            Ok(open::that(remote.get_url(BrowseOptions::MergeRequests))?)
        }
        BrowseOptions::MergeRequestId(id) => {
            let remote = remote::get(domain, path, config, false)?;
            Ok(open::that(
                remote.get_url(BrowseOptions::MergeRequestId(id)),
            )?)
        }
        BrowseOptions::Pipelines => {
            let remote = remote::get(domain, path, config, false)?;
            Ok(open::that(remote.get_url(BrowseOptions::Pipelines))?)
        }
    }
}
