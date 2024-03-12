use crate::api_traits::{Cicd, Timestamp};
use crate::cli::PipelineOptions;
use crate::config::Config;
use crate::display::{Column, DisplayBody};
use crate::remote::{ListBodyArgs, ListRemoteCliArgs};
use crate::{display, remote, Result};
use std::io::Write;
use std::sync::Arc;

use super::common::num_cicd_pages;

#[derive(Builder, Clone, Debug)]
pub struct Pipeline {
    pub status: String,
    web_url: String,
    branch: String,
    sha: String,
    created_at: String,
    updated_at: String,
}

impl Pipeline {
    pub fn builder() -> PipelineBuilder {
        PipelineBuilder::default()
    }
}

impl Timestamp for Pipeline {
    fn created_at(&self) -> String {
        self.created_at.clone()
    }
}

impl From<Pipeline> for DisplayBody {
    fn from(p: Pipeline) -> DisplayBody {
        DisplayBody {
            columns: vec![
                Column::new("URL", p.web_url),
                Column::new("Branch", p.branch),
                Column::new("SHA", p.sha),
                Column::new("Created at", p.created_at),
                Column::new("Updated at", p.updated_at),
                Column::new("Status", p.status),
            ],
        }
    }
}

#[derive(Builder, Clone)]
pub struct PipelineBodyArgs {
    pub from_to_page: Option<ListBodyArgs>,
}

impl PipelineBodyArgs {
    pub fn builder() -> PipelineBodyArgsBuilder {
        PipelineBodyArgsBuilder::default()
    }
}

pub fn execute(
    options: PipelineOptions,
    config: Arc<Config>,
    domain: String,
    path: String,
) -> Result<()> {
    match options {
        PipelineOptions::List(cli_args) => {
            let remote = remote::get_cicd(domain, path, config, cli_args.refresh_cache)?;
            if cli_args.num_pages {
                return num_cicd_pages(remote, std::io::stdout());
            }
            let from_to_args = remote::validate_from_to_page(&cli_args)?;
            let body_args = PipelineBodyArgs::builder()
                .from_to_page(from_to_args)
                .build()?;
            list_pipelines(remote, body_args, cli_args, std::io::stdout())
        }
    }
}

fn list_pipelines<W: Write>(
    remote: Arc<dyn Cicd>,
    body_args: PipelineBodyArgs,
    cli_args: ListRemoteCliArgs,
    mut writer: W,
) -> Result<()> {
    let pipelines = remote.list(body_args)?;
    if pipelines.is_empty() {
        writer.write_all(b"No pipelines found.\n")?;
        return Ok(());
    }
    display::print(
        &mut writer,
        pipelines,
        cli_args.no_headers,
        &cli_args.format,
    )?;
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::error;

    #[derive(Clone, Builder)]
    struct PipelineListMock {
        #[builder(default = "vec![]")]
        pipelines: Vec<Pipeline>,
        #[builder(default = "false")]
        error: bool,
        #[builder(setter(into, strip_option), default)]
        num_pages: Option<u32>,
    }

    impl PipelineListMock {
        pub fn builder() -> PipelineListMockBuilder {
            PipelineListMockBuilder::default()
        }
    }

    impl Cicd for PipelineListMock {
        fn list(&self, _args: PipelineBodyArgs) -> Result<Vec<Pipeline>> {
            if self.error {
                return Err(error::gen("Error"));
            }
            let pp = self.pipelines.clone();
            Ok(pp)
        }

        fn get_pipeline(&self, _id: i64) -> Result<Pipeline> {
            let pp = self.pipelines.clone();
            Ok(pp[0].clone())
        }

        fn num_pages(&self) -> Result<Option<u32>> {
            if self.error {
                return Err(error::gen("Error"));
            }
            return Ok(self.num_pages);
        }
    }

    #[test]
    fn test_list_pipelines() {
        let pp_remote = PipelineListMock::builder()
            .pipelines(vec![
                Pipeline::builder()
                    .status("success".to_string())
                    .web_url("https://gitlab.com/owner/repo/-/pipelines/123".to_string())
                    .branch("master".to_string())
                    .sha("1234567890abcdef".to_string())
                    .created_at("2020-01-01T00:00:00Z".to_string())
                    .updated_at("2020-01-01T00:01:00Z".to_string())
                    .build()
                    .unwrap(),
                Pipeline::builder()
                    .status("failed".to_string())
                    .web_url("https://gitlab.com/owner/repo/-/pipelines/456".to_string())
                    .branch("master".to_string())
                    .sha("1234567890abcdef".to_string())
                    .created_at("2020-01-01T00:00:00Z".to_string())
                    .updated_at("2020-01-01T00:01:00Z".to_string())
                    .build()
                    .unwrap(),
            ])
            .build()
            .unwrap();
        let mut buf = Vec::new();
        let body_args = PipelineBodyArgs::builder()
            .from_to_page(None)
            .build()
            .unwrap();
        let cli_args = ListRemoteCliArgs::builder().build().unwrap();
        list_pipelines(Arc::new(pp_remote), body_args, cli_args, &mut buf).unwrap();
        assert_eq!(
            String::from_utf8(buf).unwrap(),
            "URL | Branch | SHA | Created at | Updated at | Status\n\
             https://gitlab.com/owner/repo/-/pipelines/123 | master | 1234567890abcdef | 2020-01-01T00:00:00Z | 2020-01-01T00:01:00Z | success\n\
             https://gitlab.com/owner/repo/-/pipelines/456 | master | 1234567890abcdef | 2020-01-01T00:00:00Z | 2020-01-01T00:01:00Z | failed\n")
    }

    #[test]
    fn test_list_pipelines_empty() {
        let pp_remote = PipelineListMock::builder().build().unwrap();
        let mut buf = Vec::new();

        let body_args = PipelineBodyArgs::builder()
            .from_to_page(None)
            .build()
            .unwrap();
        let cli_args = ListRemoteCliArgs::builder().build().unwrap();
        list_pipelines(Arc::new(pp_remote), body_args, cli_args, &mut buf).unwrap();
        assert_eq!("No pipelines found.\n", String::from_utf8(buf).unwrap(),)
    }

    #[test]
    fn test_list_pipelines_error() {
        let pp_remote = PipelineListMock::builder().error(true).build().unwrap();
        let mut buf = Vec::new();
        let body_args = PipelineBodyArgs::builder()
            .from_to_page(None)
            .build()
            .unwrap();
        let cli_args = ListRemoteCliArgs::builder().build().unwrap();
        assert!(list_pipelines(Arc::new(pp_remote), body_args, cli_args, &mut buf).is_err());
    }

    #[test]
    fn test_list_number_of_pages() {
        let pp_remote = PipelineListMock::builder()
            .num_pages(3 as u32)
            .build()
            .unwrap();
        let mut buf = Vec::new();
        num_cicd_pages(Arc::new(pp_remote), &mut buf).unwrap();
        assert_eq!("3\n", String::from_utf8(buf).unwrap(),)
    }

    #[test]
    fn test_no_pages_available() {
        let pp_remote = PipelineListMock::builder().build().unwrap();
        let mut buf = Vec::new();
        num_cicd_pages(Arc::new(pp_remote), &mut buf).unwrap();
        assert_eq!(
            "Number of pages not available.\n",
            String::from_utf8(buf).unwrap(),
        )
    }

    #[test]
    fn test_number_of_pages_error() {
        let pp_remote = PipelineListMock::builder().error(true).build().unwrap();
        let mut buf = Vec::new();
        assert!(num_cicd_pages(Arc::new(pp_remote), &mut buf).is_err());
    }

    #[test]
    fn test_list_pipelines_no_headers() {
        let pp_remote = PipelineListMock::builder()
            .pipelines(vec![
                Pipeline::builder()
                    .status("success".to_string())
                    .web_url("https://gitlab.com/owner/repo/-/pipelines/123".to_string())
                    .branch("master".to_string())
                    .sha("1234567890abcdef".to_string())
                    .created_at("2020-01-01T00:00:00Z".to_string())
                    .updated_at("2020-01-01T00:01:00Z".to_string())
                    .build()
                    .unwrap(),
                Pipeline::builder()
                    .status("failed".to_string())
                    .web_url("https://gitlab.com/owner/repo/-/pipelines/456".to_string())
                    .branch("master".to_string())
                    .sha("1234567890abcdef".to_string())
                    .created_at("2020-01-01T00:00:00Z".to_string())
                    .updated_at("2020-01-01T00:01:00Z".to_string())
                    .build()
                    .unwrap(),
            ])
            .build()
            .unwrap();
        let mut buf = Vec::new();
        let body_args = PipelineBodyArgs::builder()
            .from_to_page(None)
            .build()
            .unwrap();
        let cli_args = ListRemoteCliArgs::builder()
            .no_headers(true)
            .build()
            .unwrap();
        list_pipelines(Arc::new(pp_remote), body_args, cli_args, &mut buf).unwrap();
        assert_eq!(
            "https://gitlab.com/owner/repo/-/pipelines/123 | master | 1234567890abcdef | 2020-01-01T00:00:00Z | 2020-01-01T00:01:00Z | success\n\
             https://gitlab.com/owner/repo/-/pipelines/456 | master | 1234567890abcdef | 2020-01-01T00:00:00Z | 2020-01-01T00:01:00Z | failed\n",
            String::from_utf8(buf).unwrap(),
        )
    }
}
