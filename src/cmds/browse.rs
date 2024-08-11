use std::sync::Arc;

use crate::cli::browse::BrowseOptions;
use crate::config::ConfigFile;
use crate::remote;
use crate::remote::CacheType;
use crate::Result;

pub fn execute(
    options: BrowseOptions,
    config: Arc<ConfigFile>,
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
            let remote = remote::get_project(domain, path, config, None, CacheType::None)?;
            Ok(open::that(remote.get_url(BrowseOptions::MergeRequests))?)
        }
        BrowseOptions::MergeRequestId(id) => {
            let remote = remote::get_project(domain, path, config, None, CacheType::None)?;
            Ok(open::that(
                remote.get_url(BrowseOptions::MergeRequestId(id)),
            )?)
        }
        BrowseOptions::Pipelines => {
            let remote = remote::get_project(domain, path, config, None, CacheType::None)?;
            Ok(open::that(remote.get_url(BrowseOptions::Pipelines))?)
        }
        BrowseOptions::PipelineId(id) => {
            let remote = remote::get_project(domain, path, config, None, CacheType::None)?;
            Ok(open::that(remote.get_url(BrowseOptions::PipelineId(id)))?)
        }
        BrowseOptions::Releases => {
            let remote = remote::get_project(domain, path, config, None, CacheType::None)?;
            Ok(open::that(remote.get_url(BrowseOptions::Releases))?)
        }
        BrowseOptions::Manual => Ok(open::that(crate::USER_GUIDE_URL)?),
    }
}
