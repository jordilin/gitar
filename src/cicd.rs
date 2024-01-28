use std::io::Write;
use std::sync::Arc;

use crate::api_traits::Cicd;
use crate::cli::{PipelineOperation, PipelineOptions};
use crate::Result;

pub fn execute<W: Write>(
    remote: Arc<dyn Cicd>,
    options: PipelineOptions,
    writer: &mut W,
) -> Result<()> {
    match options.operation {
        PipelineOperation::List => list_pipelines(remote, writer),
    }
}

fn list_pipelines<W: Write>(remote: Arc<dyn Cicd>, mut writer: W) -> Result<()> {
    let pipelines = remote.list_pipelines()?;
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
    use crate::cli::PipelineOptions;
    use crate::error;
    use crate::remote::Pipeline;

    use super::*;

    #[derive(Clone, Builder)]
    struct PipelineList {
        #[builder(default = "vec![]")]
        pipelines: Vec<Pipeline>,
        #[builder(default = "false")]
        error: bool,
    }

    impl Cicd for PipelineList {
        fn list_pipelines(&self) -> Result<Vec<Pipeline>> {
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

    #[test]
    fn test_list_pipelines() {
        let pp_remote = PipelineListBuilder::default()
            .pipelines(vec![
                Pipeline::new(
                    "success",
                    "https://gitlab.com/owner/repo/-/pipelines/123",
                    "master",
                    "1234567890abcdef",
                    "2020-01-01T00:00:00Z",
                ),
                Pipeline::new(
                    "failed",
                    "https://gitlab.com/owner/repo/-/pipelines/456",
                    "master",
                    "1234567890abcdef",
                    "2020-01-01T00:00:00Z",
                ),
            ])
            .build()
            .unwrap();
        let mut buf = Vec::new();
        let pp_options = PipelineOptions {
            operation: PipelineOperation::List,
            refresh_cache: false,
        };
        execute(Arc::new(pp_remote), pp_options, &mut buf).unwrap();
        assert_eq!(
            String::from_utf8(buf).unwrap(),
            "URL | Branch | SHA | Created at | Status\n\
             https://gitlab.com/owner/repo/-/pipelines/123 | master | 1234567890abcdef | 2020-01-01T00:00:00Z | success\n\
             https://gitlab.com/owner/repo/-/pipelines/456 | master | 1234567890abcdef | 2020-01-01T00:00:00Z | failed\n")
    }

    #[test]
    fn test_list_pipelines_empty() {
        let pp_remote = PipelineListBuilder::default().build().unwrap();
        let mut buf = Vec::new();
        list_pipelines(Arc::new(pp_remote), &mut buf).unwrap();
        assert_eq!("No pipelines found.\n", String::from_utf8(buf).unwrap(),)
    }

    #[test]
    fn test_list_pipelines_error() {
        let pp_remote = PipelineListBuilder::default().error(true).build().unwrap();
        let pp_options = PipelineOptions {
            operation: PipelineOperation::List,
            refresh_cache: false,
        };
        let mut buf = Vec::new();
        assert!(execute(Arc::new(pp_remote), pp_options, &mut buf).is_err());
    }
}
