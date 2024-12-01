use std::sync::Arc;

use env_logger::Env;
use gr::{
    cli::{
        browse::BrowseOptions, merge_request::MergeRequestOptions, parse_cli,
        trending::TrendingOptions, CliOptions,
    },
    cmds::{self, browse, cicd, docker, merge_request, project},
    init,
    remote::{self, CliDomainRequirements, ConfigFilePath, RemoteURL},
    shell::BlockingCommand,
    Result,
};

fn main() -> Result<()> {
    let option_args = parse_cli();
    let cli_options = option_args.cli_options.unwrap_or_else(|| {
        eprintln!("Please specify a subcommand");
        std::process::exit(1);
    });
    let cli_args = option_args.cli_args;
    // Default config file gitar.toml
    let config_file_path = ConfigFilePath::new(&cli_args);
    match cli_args.verbose {
        1 => env_logger::init_from_env(Env::default().default_filter_or("info")),
        2 => env_logger::init_from_env(Env::default().default_filter_or("debug")),
        _ => (),
    }
    match handle_cli_options(cli_options, config_file_path, cli_args) {
        Err(err) => {
            eprintln!("{}", err);
            std::process::exit(1);
        }
        Ok(_) => Ok(()),
    }
}

fn handle_cli_options(
    cli_options: CliOptions,
    config_file_path: ConfigFilePath,
    cli_args: gr::cli::CliArgs,
) -> Result<()> {
    match cli_options {
        CliOptions::MergeRequest(options) => {
            let url = if let MergeRequestOptions::Create(opts) = &options {
                // This is a create merge request operation. The remote URL that
                // we are targeting is either our own or the remote of our
                // fork specified with --target-repo
                let reqs = vec![CliDomainRequirements::CdInLocalRepo];
                remote::url(
                    &cli_args,
                    &reqs,
                    &BlockingCommand,
                    &opts.target_repo.as_deref(),
                )?
            } else {
                // For operations involving list, get, close, etc... it is not a
                // requirement to be in a local repo. We can do --repo <repo> to
                // list merge requests, close merge requests, etc.
                let reqs = vec![
                    CliDomainRequirements::RepoArgs,
                    CliDomainRequirements::CdInLocalRepo,
                ];
                remote::url(&cli_args, &reqs, &BlockingCommand, &None)?
            };

            let config = remote::read_config(config_file_path, &url)?;
            merge_request::execute(
                options,
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
            let url = remote::url(&cli_args, &requirements, &BlockingCommand, &None)?;
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
            let url = remote::url(&cli_args, &requirements, &BlockingCommand, &None)?;
            let config = remote::read_config(config_file_path, &url)?;
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
            let url = remote::url(&cli_args, &requirements, &BlockingCommand, &None)?;
            let config = remote::read_config(config_file_path, &url)?;
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
            let url = remote::url(&cli_args, &requirements, &BlockingCommand, &None)?;
            let config = remote::read_config(config_file_path, &url)?;
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
            let url = remote::url(&cli_args, &requirements, &BlockingCommand, &None)?;
            let config = remote::read_config(config_file_path, &url)?;
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
            let url = remote::url(&cli_args, &requirements, &BlockingCommand, &None)?;
            let config = remote::read_config(config_file_path, &url)?;
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
                let config = remote::read_config(config_file_path, &url)?;
                cmds::trending::execute(args, config, domain)
            }
        },
        CliOptions::Init(options) => init::execute(options, config_file_path),
        CliOptions::Cache(options) => {
            let requirements = vec![
                CliDomainRequirements::DomainArgs,
                CliDomainRequirements::RepoArgs,
                CliDomainRequirements::CdInLocalRepo,
            ];
            let url = remote::url(&cli_args, &requirements, &BlockingCommand, &None)?;
            let config = remote::read_config(config_file_path, &url)?;
            cmds::cache::execute(options, config)
        }
        CliOptions::Manual => browse::execute(
            BrowseOptions::Manual,
            Arc::new(gr::config::ConfigFile::default()),
            "".to_string(),
            "".to_string(),
        ),
        CliOptions::Amps(options) => cmds::amps::execute(options, config_file_path),
        CliOptions::User(options) => {
            let requirements = vec![
                CliDomainRequirements::DomainArgs,
                CliDomainRequirements::RepoArgs,
                CliDomainRequirements::CdInLocalRepo,
            ];
            let url = remote::url(&cli_args, &requirements, &BlockingCommand, &None)?;
            let config = remote::read_config(config_file_path, &url)?;
            cmds::user::execute(
                options,
                config,
                url.domain().to_string(),
                url.path().to_string(),
            )
        }
    }
}
