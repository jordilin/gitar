use std::io::Write;
use std::sync::Arc;

use crate::api_traits::TrendingProjectURL;
use crate::config::Config;
use crate::display::{Column, DisplayBody};
use crate::remote::{self, GetRemoteCliArgs};
use crate::Result;

use super::common;

pub struct TrendingCliArgs {
    pub domain: String,
    pub language: String,
    pub get_args: GetRemoteCliArgs,
    // Used for macro compatibility when listing resources during display.
    pub flush: bool,
}

#[derive(Clone)]
pub struct TrendingProject {
    pub url: String,
}

impl TrendingProject {
    pub fn new(url: String) -> Self {
        Self { url }
    }
}

impl From<TrendingProject> for DisplayBody {
    fn from(url: TrendingProject) -> Self {
        DisplayBody::new(vec![Column::new("URL", url.url)])
    }
}

pub fn execute(cli_args: TrendingCliArgs, config: Arc<Config>) -> Result<()> {
    let remote = remote::get_trending(
        cli_args.domain.clone(),
        // does not matter in this command. Implementing it for
        // Github.com which is just a query against HTML page.
        "".to_string(),
        config,
        cli_args.get_args.refresh_cache,
    )?;
    get_urls(remote, cli_args, &mut std::io::stdout())
}

fn get_urls<W: Write>(
    remote: Arc<dyn TrendingProjectURL>,
    cli_args: TrendingCliArgs,
    writer: &mut W,
) -> Result<()> {
    common::list_trending(remote, cli_args.language.to_string(), cli_args, writer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct MockTrendingProjectURL {
        projects: Vec<TrendingProject>,
    }

    impl MockTrendingProjectURL {
        fn new(projects: Vec<TrendingProject>) -> Self {
            Self { projects }
        }
    }

    impl TrendingProjectURL for MockTrendingProjectURL {
        fn list(&self, _language: String) -> Result<Vec<TrendingProject>> {
            Ok(self.projects.clone())
        }
    }

    #[test]
    fn test_no_urls() {
        let remote = Arc::new(MockTrendingProjectURL::default());
        let cli_args = TrendingCliArgs {
            domain: "gitlab".to_string(),
            language: "rust".to_string(),
            get_args: GetRemoteCliArgs::builder().build().unwrap(),
            flush: false,
        };
        let mut buf = Vec::new();
        get_urls(remote, cli_args, &mut buf).unwrap();
        assert_eq!("No resources found.\n", String::from_utf8(buf).unwrap(),)
    }

    #[test]
    fn test_trending_projects() {
        let projects = vec![
            TrendingProject::new("https://github.com/kubernetes/kubernetes".to_string()),
            TrendingProject::new("https://github.com/jordilin/gitar".to_string()),
        ];
        let remote = Arc::new(MockTrendingProjectURL::new(projects));
        let cli_args = TrendingCliArgs {
            domain: "gitlab".to_string(),
            language: "rust".to_string(),
            get_args: GetRemoteCliArgs::builder().build().unwrap(),
            flush: false,
        };
        let mut buf = Vec::new();
        get_urls(remote, cli_args, &mut buf).unwrap();
        assert_eq!(
            "URL\nhttps://github.com/kubernetes/kubernetes\nhttps://github.com/jordilin/gitar\n",
            String::from_utf8(buf).unwrap(),
        )
    }
}
