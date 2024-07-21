use std::{path::Path, sync::Arc};

use env_logger::Env;
use gr::{
    cli::{browse::BrowseOptions, parse_cli, trending::TrendingOptions, CliOptions},
    cmds::{self, browse, cicd, docker, merge_request, project},
    init,
    remote::{self, CliDomainRequirements},
    shell::BlockingCommand,
    Result,
};

const DEFAULT_CONFIG_PATH: &str = ".config/gitar/api";

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
            let (domain, path) =
                remote::get_domain_path(&cli_args, &requirements, &BlockingCommand)?;
            let config = remote::read_config(&config_file, &domain)?;
            merge_request::execute(options, cli_args, config, domain, path)
        }
        CliOptions::Browse(options) => {
            // Use default config for browsing - does not require auth.
            let config = Arc::new(gr::config::Config::default());
            let requirements = vec![
                CliDomainRequirements::RepoArgs,
                CliDomainRequirements::CdInLocalRepo,
            ];
            let (domain, path) =
                remote::get_domain_path(&cli_args, &requirements, &BlockingCommand)?;
            browse::execute(options, config, domain, path)
        }
        CliOptions::Pipeline(options) => {
            let requirements = vec![
                CliDomainRequirements::RepoArgs,
                CliDomainRequirements::CdInLocalRepo,
            ];
            let (domain, path) =
                remote::get_domain_path(&cli_args, &requirements, &BlockingCommand)?;
            let config = remote::read_config(&config_file, &domain)?;
            cicd::execute(options, config, domain, path)
        }
        CliOptions::Project(options) => {
            let requirements = vec![
                CliDomainRequirements::RepoArgs,
                CliDomainRequirements::CdInLocalRepo,
            ];
            let (domain, path) =
                remote::get_domain_path(&cli_args, &requirements, &BlockingCommand)?;
            let config = remote::read_config(&config_file, &domain)?;
            project::execute(options, config, domain, path)
        }
        CliOptions::Docker(options) => {
            let requirements = vec![
                CliDomainRequirements::RepoArgs,
                CliDomainRequirements::CdInLocalRepo,
            ];
            let (domain, path) =
                remote::get_domain_path(&cli_args, &requirements, &BlockingCommand)?;
            let config = remote::read_config(&config_file, &domain)?;
            docker::execute(options, config, domain, path)
        }
        CliOptions::Release(options) => {
            let requirements = vec![
                CliDomainRequirements::RepoArgs,
                CliDomainRequirements::CdInLocalRepo,
            ];
            let (domain, path) =
                remote::get_domain_path(&cli_args, &requirements, &BlockingCommand)?;
            let config = remote::read_config(&config_file, &domain)?;
            cmds::release::execute(options, config, domain, path)
        }
        CliOptions::My(options) => {
            let requirements = vec![
                CliDomainRequirements::DomainArgs,
                CliDomainRequirements::CdInLocalRepo,
            ];
            let (domain, path) =
                remote::get_domain_path(&cli_args, &requirements, &BlockingCommand)?;
            let config = remote::read_config(&config_file, &domain)?;
            cmds::my::execute(options, config, domain, path)
        }
        CliOptions::Trending(options) => match options {
            TrendingOptions::Get(args) => {
                // Trending repos is for github.com - Allow for `gr tr
                // <language>` everywhere in the shell.
                let domain = "github.com";
                let config = remote::read_config(&config_file, domain)?;
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
            let (domain, _) = remote::get_domain_path(&cli_args, &requirements, &BlockingCommand)?;
            let config = remote::read_config(&config_file, &domain)?;
            cmds::cache::execute(options, config)
        }
        CliOptions::Manual => browse::execute(
            BrowseOptions::Manual,
            Arc::new(gr::config::Config::default()),
            "".to_string(),
            "".to_string(),
        ),
        CliOptions::Amps(options) => cmds::amps::execute(options, config_file),
    }
}
