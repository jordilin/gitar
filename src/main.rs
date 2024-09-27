use std::{path::Path, sync::Arc};

use env_logger::Env;
use gr::{
    cli::{browse::BrowseOptions, parse_cli, trending::TrendingOptions, CliOptions},
    cmds::{self, browse, cicd, docker, merge_request, project},
    init,
    remote::{self, CliDomainRequirements, RemoteURL},
    shell::BlockingCommand,
    Result,
};

const DEFAULT_CONFIG_PATH: &str = ".config/gitar/gitar.toml";

fn main() -> Result<()> {
    let home_dir = std::env::var("HOME").unwrap();
    let option_args = parse_cli();
    let cli_options = option_args.cli_options.unwrap_or_else(|| {
        eprintln!("Please specify a subcommand");
        std::process::exit(1);
    });
    let cli_args = option_args.cli_args;
    let mut config_file = Path::new(&home_dir).join(DEFAULT_CONFIG_PATH);
    if let Some(ref config) = cli_args.config {
        config_file = Path::new(&config).to_path_buf();
    }
    match cli_args.verbose {
        1 => env_logger::init_from_env(Env::default().default_filter_or("info")),
        2 => env_logger::init_from_env(Env::default().default_filter_or("debug")),
        _ => (),
    }
    match handle_cli_options(cli_options, config_file, cli_args) {
        Err(err) => {
            eprintln!("{}", err);
            std::process::exit(1);
        }
        Ok(_) => Ok(()),
    }
}

fn handle_cli_options(
    cli_options: CliOptions,
    config_file: std::path::PathBuf,
    cli_args: gr::cli::CliArgs,
) -> Result<()> {
    match cli_options {
        CliOptions::MergeRequest(options) => {
            let requirements = vec![
                CliDomainRequirements::RepoArgs,
                CliDomainRequirements::CdInLocalRepo,
            ];
            let url = remote::url(&cli_args, &requirements, &BlockingCommand)?;
            let config = remote::read_config(&config_file, &url)?;
            merge_request::execute(
                options,
                cli_args,
                config,
                url.domain().to_string(),
                url.path().to_string(),
            )
        }
        CliOptions::Browse(options) => {
            // Use default config for browsing - does not require auth.
            let config = Arc::new(gr::config::ConfigFile::default());
            let requirements = vec![
                CliDomainRequirements::RepoArgs,
                CliDomainRequirements::CdInLocalRepo,
            ];
            let url = remote::url(&cli_args, &requirements, &BlockingCommand)?;
            browse::execute(
                options,
                config,
                url.domain().to_string(),
                url.path().to_string(),
            )
        }
        CliOptions::Pipeline(options) => {
            let requirements = vec![
                CliDomainRequirements::RepoArgs,
                CliDomainRequirements::CdInLocalRepo,
            ];
            let url = remote::url(&cli_args, &requirements, &BlockingCommand)?;
            let config = remote::read_config(&config_file, &url)?;
            cicd::execute(
                options,
                config,
                url.domain().to_string(),
                url.path().to_string(),
            )
        }
        CliOptions::Project(options) => {
            let requirements = vec![
                CliDomainRequirements::RepoArgs,
                CliDomainRequirements::CdInLocalRepo,
            ];
            let url = remote::url(&cli_args, &requirements, &BlockingCommand)?;
            let config = remote::read_config(&config_file, &url)?;
            project::execute(
                options,
                config,
                url.domain().to_string(),
                url.path().to_string(),
            )
        }
        CliOptions::Docker(options) => {
            let requirements = vec![
                CliDomainRequirements::RepoArgs,
                CliDomainRequirements::CdInLocalRepo,
            ];
            let url = remote::url(&cli_args, &requirements, &BlockingCommand)?;
            let config = remote::read_config(&config_file, &url)?;
            docker::execute(
                options,
                config,
                url.domain().to_string(),
                url.path().to_string(),
            )
        }
        CliOptions::Release(options) => {
            let requirements = vec![
                CliDomainRequirements::RepoArgs,
                CliDomainRequirements::CdInLocalRepo,
            ];
            let url = remote::url(&cli_args, &requirements, &BlockingCommand)?;
            let config = remote::read_config(&config_file, &url)?;
            cmds::release::execute(
                options,
                config,
                url.domain().to_string(),
                url.path().to_string(),
            )
        }
        CliOptions::My(options) => {
            let requirements = vec![
                CliDomainRequirements::DomainArgs,
                CliDomainRequirements::CdInLocalRepo,
            ];
            let url = remote::url(&cli_args, &requirements, &BlockingCommand)?;
            let config = remote::read_config(&config_file, &url)?;
            cmds::my::execute(
                options,
                config,
                url.domain().to_string(),
                url.path().to_string(),
            )
        }
        CliOptions::Trending(options) => match options {
            TrendingOptions::Get(args) => {
                // Trending repos is for github.com - Allow for `gr tr
                // <language>` everywhere in the shell.
                let domain = "github.com";
                let url = RemoteURL::new(domain.to_string(), "".to_string());
                let config = remote::read_config(&config_file, &url)?;
                cmds::trending::execute(args, config, domain)
            }
        },
        CliOptions::Init(options) => init::execute(options, config_file),
        CliOptions::Cache(options) => {
            let requirements = vec![
                CliDomainRequirements::DomainArgs,
                CliDomainRequirements::RepoArgs,
                CliDomainRequirements::CdInLocalRepo,
            ];
            let url = remote::url(&cli_args, &requirements, &BlockingCommand)?;
            let config = remote::read_config(&config_file, &url)?;
            cmds::cache::execute(options, config)
        }
        CliOptions::Manual => browse::execute(
            BrowseOptions::Manual,
            Arc::new(gr::config::ConfigFile::default()),
            "".to_string(),
            "".to_string(),
        ),
        CliOptions::Amps(options) => cmds::amps::execute(options, config_file),
        CliOptions::User(options) => {
            let requirements = vec![
                CliDomainRequirements::DomainArgs,
                CliDomainRequirements::RepoArgs,
                CliDomainRequirements::CdInLocalRepo,
            ];
            let url = remote::url(&cli_args, &requirements, &BlockingCommand)?;
            let config = remote::read_config(&config_file, &url)?;
            cmds::user::execute(
                options,
                config,
                url.domain().to_string(),
                url.path().to_string(),
            )
        }
    }
}
