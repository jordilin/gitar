use crate::cache::filesystem::FileCache;
use crate::cli::BrowseOptions;
use crate::config::Config;
use crate::http;
use crate::remote;
use crate::Result;
use std::sync::Arc;

pub fn execute(options: BrowseOptions, config: Config, domain: String, path: String) -> Result<()> {
    match options {
        BrowseOptions::Repo => {
            // No need to contact the remote object, domain and path already
            // computed.
            let remote_url = format!("https://{}/{}", domain, path);
            Ok(open::that(remote_url)?)
        }
        BrowseOptions::MergeRequests => {
            let runner = Arc::new(http::Client::new(FileCache::new(config.clone()), false));
            let remote = remote::get(domain, path, config, runner)?;
            Ok(open::that(remote.get_url(BrowseOptions::MergeRequests))?)
        }
        BrowseOptions::MergeRequestId(id) => {
            let runner = Arc::new(http::Client::new(FileCache::new(config.clone()), false));
            let remote = remote::get(domain, path, config, runner)?;
            Ok(open::that(
                remote.get_url(BrowseOptions::MergeRequestId(id)),
            )?)
        }
        BrowseOptions::Pipelines => {
            let runner = Arc::new(http::Client::new(FileCache::new(config.clone()), false));
            let remote = remote::get(domain, path, config, runner)?;
            Ok(open::that(remote.get_url(BrowseOptions::Pipelines))?)
        }
    }
}
