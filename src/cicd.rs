use crate::api_traits::{ApiOperation, Cicd, QueryPages};
use crate::cli::PipelineOptions;
use crate::config::Config;
use crate::remote::PipelineBodyArgs;
use crate::{remote, Result};
use std::io::Write;
use std::sync::Arc;

pub fn execute(
    options: PipelineOptions,
    config: Arc<Config>,
    domain: String,
    path: String,
) -> Result<()> {
    match options {
        PipelineOptions::List(cli_args) => {
            if cli_args.num_pages {
                let remote = remote::get_list_pages(domain, path, config, cli_args.refresh_cache)?;
                return query_pages(remote, std::io::stdout());
            }
            let remote = remote::get_cicd(domain, path, config, cli_args.refresh_cache)?;
            let from_to_args = remote::validate_from_to_page(&cli_args)?;
            let body_args = PipelineBodyArgs::builder()
                .from_to_page(from_to_args)
                .build()?;
            list_pipelines(remote, body_args, std::io::stdout())
        }
    }
}

fn query_pages<W: Write>(remote: Arc<dyn QueryPages>, mut writer: W) -> Result<()> {
    match remote.num_pages(&ApiOperation::Pipeline) {
        Ok(Some(pages)) => writer.write_all(format!("{pages}\n", pages = pages).as_bytes())?,
        Ok(None) => {
            writer.write_all(b"Number of pages not available.\n")?;
        }
        Err(e) => {
            return Err(e);
        }
    };
    Ok(())
}

fn list_pipelines<W: Write>(
    remote: Arc<dyn Cicd>,
    body_args: PipelineBodyArgs,
    mut writer: W,
) -> Result<()> {
    let pipelines = remote.list(body_args)?;
    if pipelines.is_empty() {
        writer.write_all(b"No pipelines found.\n")?;
        return Ok(());
    }
    writer.write_all(b"URL | Branch | SHA | Created at | Status\n")?;
    for pipeline in pipelines {
        writer.write_all(format!("{}\n", pipeline).as_bytes())?;
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use crate::error;
    use crate::remote::Pipeline;

    use super::*;

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
    }

    impl QueryPages for PipelineListMock {
        fn num_pages(&self, _op: &ApiOperation) -> Result<Option<u32>> {
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
                    .build()
                    .unwrap(),
                Pipeline::builder()
                    .status("failed".to_string())
                    .web_url("https://gitlab.com/owner/repo/-/pipelines/456".to_string())
                    .branch("master".to_string())
                    .sha("1234567890abcdef".to_string())
                    .created_at("2020-01-01T00:00:00Z".to_string())
                    .build()
                    .unwrap(),
            ])
            .build()
            .unwrap();
        let mut buf = Vec::new();
        let args = PipelineBodyArgs::builder()
            .from_to_page(None)
            .build()
            .unwrap();
        list_pipelines(Arc::new(pp_remote), args, &mut buf).unwrap();
        assert_eq!(
            String::from_utf8(buf).unwrap(),
            "URL | Branch | SHA | Created at | Status\n\
             https://gitlab.com/owner/repo/-/pipelines/123 | master | 1234567890abcdef | 2020-01-01T00:00:00Z | success\n\
             https://gitlab.com/owner/repo/-/pipelines/456 | master | 1234567890abcdef | 2020-01-01T00:00:00Z | failed\n")
    }

    #[test]
    fn test_list_pipelines_empty() {
        let pp_remote = PipelineListMock::builder().build().unwrap();
        let mut buf = Vec::new();
        let args = PipelineBodyArgs::builder()
            .from_to_page(None)
            .build()
            .unwrap();
        list_pipelines(Arc::new(pp_remote), args, &mut buf).unwrap();
        assert_eq!("No pipelines found.\n", String::from_utf8(buf).unwrap(),)
    }

    #[test]
    fn test_list_pipelines_error() {
        let pp_remote = PipelineListMock::builder().error(true).build().unwrap();
        let mut buf = Vec::new();
        let args = PipelineBodyArgs::builder()
            .from_to_page(None)
            .build()
            .unwrap();
        assert!(list_pipelines(Arc::new(pp_remote), args, &mut buf).is_err());
    }

    #[test]
    fn test_list_number_of_pages() {
        let pp_remote = PipelineListMock::builder()
            .num_pages(3 as u32)
            .build()
            .unwrap();
        let mut buf = Vec::new();
        query_pages(Arc::new(pp_remote), &mut buf).unwrap();
        assert_eq!("3\n", String::from_utf8(buf).unwrap(),)
    }

    #[test]
    fn test_no_pages_available() {
        let pp_remote = PipelineListMock::builder().build().unwrap();
        let mut buf = Vec::new();
        query_pages(Arc::new(pp_remote), &mut buf).unwrap();
        assert_eq!(
            "Number of pages not available.\n",
            String::from_utf8(buf).unwrap(),
        )
    }

    #[test]
    fn test_number_of_pages_error() {
        let pp_remote = PipelineListMock::builder().error(true).build().unwrap();
        let mut buf = Vec::new();
        assert!(query_pages(Arc::new(pp_remote), &mut buf).is_err());
    }
}
