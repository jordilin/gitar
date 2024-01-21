use crate::cli::ProjectOptions;
use crate::config::Config;
use crate::error;
use crate::io::CmdInfo;
use crate::remote;
use crate::Result;
use std::sync::Arc;

pub fn execute(
    options: ProjectOptions,
    config: Arc<Config>,
    domain: String,
    path: String,
) -> Result<()> {
    let remote = remote::get(domain, path, config, false)?;
    match options {
        ProjectOptions::Info { id } => {
            let CmdInfo::Project(project_data) = remote.get_project_data(id)? else {
                return Err(error::gen("No project data found."));
            };
            println!("{}", project_data);
        }
    }
    Ok(())
}
