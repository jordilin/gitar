use crate::cli::PipelineOptions;
use crate::config::Config;
use crate::remote;
use crate::Result;

pub fn execute(
    options: PipelineOptions,
    config: Config,
    domain: String,
    path: String,
) -> Result<()> {
    match options {
        PipelineOptions::List { refresh_cache } => {
            let remote = remote::get(domain, path, config, refresh_cache)?;
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
