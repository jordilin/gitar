use std::{fs::File, path::Path, sync::Arc};

use gr::{
    api_traits::Remote,
    cache::filesystem::FileCache,
    cli::{parse_cli, BrowseOptions, CliOptions, MergeRequestOptions},
    config::Config,
    error, git,
    github::Github,
    gitlab::Gitlab,
    http,
    io::{CmdInfo, HttpRunner, Response},
    merge_request,
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

    match cli_options {
        CliOptions::MergeRequest(mr_options) => {
            let config = gr::config::Config::new(f, &domain).expect("Unable to read config");
            match mr_options {
                MergeRequestOptions::Create {
                    title,
                    description,
                    noprompt,
                    refresh_cache,
                } => {
                    let runner = Arc::new(http::Client::new(
                        FileCache::new(config.clone()),
                        refresh_cache,
                    ));
                    let remote = get_remote(domain, path, config.clone(), runner)?;
                    merge_request::open(remote, Arc::new(config), title, description, noprompt)
                }
                MergeRequestOptions::List {
                    state,
                    refresh_cache,
                } => {
                    let runner = Arc::new(http::Client::new(
                        FileCache::new(config.clone()),
                        refresh_cache,
                    ));
                    let remote = get_remote(domain, path, config, runner)?;
                    merge_request::list(remote, state)
                }
                MergeRequestOptions::Merge { id } => {
                    let runner = Arc::new(http::Client::new(FileCache::new(config.clone()), false));
                    let remote = get_remote(domain, path, config, runner)?;
                    merge_request::merge(remote, id)
                }
                MergeRequestOptions::Checkout { id } => {
                    let runner = Arc::new(http::Client::new(FileCache::new(config.clone()), false));
                    let remote = get_remote(domain, path, config, runner)?;
                    merge_request::checkout(remote, id)
                }
                MergeRequestOptions::Close { id } => {
                    let runner = Arc::new(http::Client::new(FileCache::new(config.clone()), false));
                    let remote = get_remote(domain, path, config, runner)?;
                    merge_request::close(remote, id)
                }
            }
        }
        CliOptions::Browse(options) => {
            let config = gr::config::Config::default();
            match options {
                BrowseOptions::Repo => {
                    // No need to contact the remote object, domain and path already
                    // computed.
                    let remote_url = format!("https://{}/{}", domain, path);
                    Ok(open::that(remote_url)?)
                }
                BrowseOptions::MergeRequests => {
                    let runner = Arc::new(http::Client::new(FileCache::new(config.clone()), false));
                    let remote = get_remote(domain, path, config, runner)?;
                    Ok(open::that(remote.get_url(BrowseOptions::MergeRequests))?)
                }
                BrowseOptions::MergeRequestId(id) => {
                    let runner = Arc::new(http::Client::new(FileCache::new(config.clone()), false));
                    let remote = get_remote(domain, path, config, runner)?;
                    Ok(open::that(
                        remote.get_url(BrowseOptions::MergeRequestId(id)),
                    )?)
                }
            }
        }
    }
}

fn get_remote<T: HttpRunner<Response = Response> + Send + Sync + 'static>(
    domain: String,
    path: String,
    config: Config,
    runner: Arc<T>,
) -> Result<Arc<dyn Remote>> {
    let github_domain_regex = regex::Regex::new(r"^github").unwrap();
    let gitlab_domain_regex = regex::Regex::new(r"^gitlab").unwrap();

    let remote: Arc<dyn Remote> = if github_domain_regex.is_match(&domain) {
        Arc::new(Github::new(config, &domain, &path, runner))
    } else if gitlab_domain_regex.is_match(&domain) {
        Arc::new(Gitlab::new(config, &domain, &path, runner))
    } else {
        return Err(error::gen(format!("Unsupported domain: {}", &domain)));
    };
    Ok(remote)
}
