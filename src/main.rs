use std::{fs::File, path::Path, sync::Arc};

use env_logger::Env;
use gr::{
    cli::{parse_cli, trending::TrendingOptions, CliOptions},
    cmds::{self, browse, cicd, docker, merge_request, project},
    error::GRError,
    git, init,
    io::CmdInfo,
    remote::get_domain_path,
    shell::Shell,
    Result,
};

const CONFIG_PATH: &str = ".config/gitar/api";

fn get_config_domain_path(
    config_file: &Path,
    cli_args: &gr::cli::CliArgs,
    requires_local_repo: bool,
    read_config: bool,
) -> Result<(Arc<gr::config::Config>, String, String)> {
    let (domain, path) = if cli_args.repo.is_some() {
        get_domain_path(cli_args.repo.as_ref().unwrap())
    } else if requires_local_repo {
        match git::remote_url(&Shell) {
            Ok(CmdInfo::RemoteUrl { domain, path }) => (domain, path),
            Err(err) => {
                return Err(GRError::GitRemoteUrlNotFound(format!(
                    "{}: {}",
                    err, "Unable to get the remote url."
                ))
                .into())
            }
            _ => {
                return Err(GRError::ApplicationError(
                    "Could not get remote url during startup. \
                    main::get_config_domain_path - Please open a bug to \
                    https://github.com/jordilin/gitar"
                        .to_string(),
                )
                .into())
            }
        }
    } else if cli_args.domain.is_some() {
        (
            cli_args.domain.as_ref().unwrap().to_string(),
            "".to_string(),
        )
    } else {
        return Err(
            GRError::DomainOrRepoExpected("Missing repository information".to_string()).into(),
        );
    };
    if read_config {
        let f = File::open(config_file).expect("Unable to open config file");
        let config = Arc::new(gr::config::Config::new(f, &domain).expect("Unable to read config"));
        Ok((config, domain, path))
    } else {
        Ok((Arc::new(gr::config::Config::default()), domain, path))
    }
}

fn main() -> Result<()> {
    let home_dir = std::env::var("HOME").unwrap();
    let config_file = Path::new(&home_dir).join(CONFIG_PATH);
    let option_args = parse_cli();
    let cli_options = option_args.cli_options.unwrap_or_else(|| {
        eprintln!("Please specify a subcommand");
        std::process::exit(1);
    });
    let cli_args = option_args.cli_args;
    if cli_args.verbose {
        let env = Env::default().default_filter_or("info");
        env_logger::init_from_env(env);
    }
    match handle_cli_options(cli_options, config_file, cli_args) {
        Err(err) => match err.downcast_ref::<GRError>() {
            Some(GRError::GitRemoteUrlNotFound(msg)) => {
                eprintln!(
                    "Please cd into a git repository or set --repo option - {}",
                    msg
                );
                std::process::exit(1);
            }
            Some(GRError::DomainOrRepoExpected(msg)) => {
                eprintln!(
                    "Please set a domain with --domain flag or cd \
                into a git repository or set --repo option - {}",
                    msg
                );
                std::process::exit(1);
            }
            _ => {
                eprintln!("{}", err);
                std::process::exit(1);
            }
        },
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
            let (config, domain, path) =
                get_config_domain_path(&config_file, &cli_args, true, true)?;
            merge_request::execute(options, cli_args, config, domain, path)
        }
        CliOptions::Browse(options) => {
            // Use default config for browsing - does not require auth.
            let (config, domain, path) =
                get_config_domain_path(&config_file, &cli_args, true, false)?;
            browse::execute(options, config, domain, path)
        }
        CliOptions::Pipeline(options) => {
            let (config, domain, path) =
                get_config_domain_path(&config_file, &cli_args, true, true)?;
            cicd::execute(options, config, domain, path)
        }
        CliOptions::Project(options) => {
            let (config, domain, path) =
                get_config_domain_path(&config_file, &cli_args, true, true)?;
            project::execute(options, config, domain, path)
        }
        CliOptions::Docker(options) => {
            let (config, domain, path) =
                get_config_domain_path(&config_file, &cli_args, true, true)?;
            docker::execute(options, config, domain, path)
        }
        CliOptions::Release(options) => {
            let (config, domain, path) =
                get_config_domain_path(&config_file, &cli_args, true, true)?;
            cmds::release::execute(options, config, domain, path)
        }
        CliOptions::My(options) => {
            let (config, domain, path) =
                get_config_domain_path(&config_file, &cli_args, false, true)?;
            cmds::my::execute(options, config, domain, path)
        }
        CliOptions::Trending(options) => match options {
            TrendingOptions::Get(args) => {
                let (config, domain, _) =
                    get_config_domain_path(&config_file, &cli_args, false, true)?;
                cmds::trending::execute(args, config, domain)
            }
        },
        CliOptions::Init(options) => init::execute(options, config_file),
    }
}
