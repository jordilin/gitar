use std::{fs::File, path::Path, sync::Arc};

use gr::{
    browse, cicd,
    cli::{parse_cli, CliOptions},
    cmds,
    cmds::merge_request,
    docker, error, git, init,
    io::CmdInfo,
    project,
    shell::Shell,
    Result,
};

const CONFIG_PATH: &str = ".config/gitar/api";

fn main() -> Result<()> {
    let home_dir = std::env::var("HOME").unwrap();
    let config_file = Path::new(&home_dir).join(CONFIG_PATH);
    let cli_options = parse_cli().unwrap_or_else(|| {
        eprintln!("Please specify a subcommand");
        std::process::exit(1);
    });
    if let CliOptions::Init(options) = cli_options {
        init::execute(options, config_file)
    } else {
        let f = File::open(config_file).expect("Unable to open file");
        let CmdInfo::RemoteUrl { domain, path } = git::remote_url(&Shell)? else {
            return Err(error::gen("No remote url found. Please set a remote url."));
        };
        let config = Arc::new(gr::config::Config::new(f, &domain).expect("Unable to read config"));
        match cli_options {
            CliOptions::MergeRequest(options) => {
                merge_request::execute(options, config, domain, path)
            }
            CliOptions::Browse(options) => {
                // Use default config for browsing - does not require auth.
                let config = Arc::new(gr::config::Config::default());
                browse::execute(options, config, domain, path)
            }
            CliOptions::Pipeline(options) => cicd::execute(options, config, domain, path),
            CliOptions::Project(options) => project::execute(options, config, domain, path),
            CliOptions::Docker(options) => docker::execute(options, config, domain, path),
            CliOptions::Release(options) => cmds::release::execute(options, config, domain, path),
            // Init command is handled above when user creates a new
            // configuration - this is unreachable
            CliOptions::Init(_) => unreachable!(),
        }
    }
}
