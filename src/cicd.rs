use crate::cache::filesystem::FileCache;
use crate::cli::PipelineOptions;
use crate::config::Config;
use crate::http;
use crate::remote;
use crate::Result;
use std::sync::Arc;

pub fn execute(
    options: PipelineOptions,
    config: Config,
    domain: String,
    path: String,
) -> Result<()> {
    match options {
        PipelineOptions::List { refresh_cache } => {
            let runner = Arc::new(http::Client::new(
                FileCache::new(config.clone()),
                refresh_cache,
            ));
            let remote = remote::get(domain, path, config, runner)?;
            let pipelines = remote.list_pipelines()?;
            if pipelines.is_empty() {
                println!("No pipelines found.");
                return Ok(());
            }
            println!("URL | Branch | SHA | Created at | Status");
            for pipeline in pipelines {
                println!("{}", pipeline);
            }
            Ok(())
        }
    }
}
