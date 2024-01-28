use std::{fs::File, path::Path, sync::Arc};

use gr::{
    browse, cicd,
    cli::{parse_cli, CliOptions},
    error, git,
    io::CmdInfo,
    merge_request, project, remote,
    shell::Shell,
    Result,
};

fn main() -> Result<()> {
    let home_dir = std::env::var("HOME").unwrap();
    let config_file = Path::new(&home_dir).join(".config/gitar/api");
    let f = File::open(config_file).expect("Unable to open file");
    let cli_options = parse_cli().unwrap_or_else(|| {
        eprintln!("Please specify a subcommand");
        std::process::exit(1);
    });
    let CmdInfo::RemoteUrl { domain, path } = git::remote_url(&Shell)? else {
        return Err(error::gen("No remote url found. Please set a remote url."));
    };
    let config = Arc::new(gr::config::Config::new(f, &domain).expect("Unable to read config"));
    match cli_options {
        CliOptions::MergeRequest(options) => merge_request::execute(options, config, domain, path),
        CliOptions::Browse(options) => {
            // Use default config for browsing - does not require auth.
            let config = Arc::new(gr::config::Config::default());
            browse::execute(options, config, domain, path)
        }
        CliOptions::Pipeline(options) => {
            let remote = remote::get_cicd(domain, path, config, options.refresh_cache)?;
            cicd::execute(remote, options, &mut std::io::stdout())
        }
        CliOptions::Project(options) => {
            let remote = remote::get_project(domain, path, config, options.refresh_cache)?;
            project::execute(remote, options, &mut std::io::stdout())
        }
    }
}
