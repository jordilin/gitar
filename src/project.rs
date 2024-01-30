use crate::api_traits::RemoteProject;
use crate::cli::ProjectOperation;
use crate::cli::ProjectOptions;
use crate::config::Config;
use crate::error;
use crate::io::CmdInfo;
use crate::remote;
use crate::Result;
use std::io::Write;
use std::sync::Arc;

pub fn execute(
    options: ProjectOptions,
    config: Arc<Config>,
    domain: String,
    path: String,
) -> Result<()> {
    match options.operation {
        ProjectOperation::Info { id } => {
            let remote = remote::get_project(domain, path, config, options.refresh_cache)?;
            return project_info(remote, std::io::stdout(), id);
        }
    }
}

fn project_info<W: Write>(
    remote: Arc<dyn RemoteProject>,
    mut writer: W,
    id: Option<i64>,
) -> Result<()> {
    let CmdInfo::Project(project_data) = remote.get_project_data(id)? else {
        return Err(error::GRError::ApplicationError(
            "remote.get_project_data expects CmdInfo::Project invariant".to_string(),
        )
        .into());
    };
    writer.write_all(format!("{}\n", project_data).as_bytes())?;
    Ok(())
}

#[cfg(test)]
mod test {

    use super::*;
    use crate::remote::Project;

    #[derive(Builder)]
    struct ProjectDataProvider {
        #[builder(default = "false")]
        error: bool,
        cmd_info: CmdInfo,
    }

    impl RemoteProject for ProjectDataProvider {
        fn get_project_data(&self, _id: Option<i64>) -> crate::Result<CmdInfo> {
            if self.error {
                return Err(error::gen("Error"));
            }
            match self.cmd_info {
                CmdInfo::Project(_) => Ok(self.cmd_info.clone()),
                _ => Ok(CmdInfo::Ignore),
            }
        }

        fn get_project_members(&self) -> crate::Result<CmdInfo> {
            todo!()
        }

        fn get_url(&self, _option: crate::cli::BrowseOptions) -> String {
            todo!()
        }
    }

    #[test]
    fn test_project_data_gets_persisted() {
        let remote = ProjectDataProviderBuilder::default()
            .cmd_info(CmdInfo::Project(Project::default()))
            .build()
            .unwrap();
        let remote = Arc::new(remote);
        let mut writer = Vec::new();
        project_info(remote, &mut writer, Some(1)).unwrap();
        assert!(writer.len() > 0);
    }

    #[test]
    fn test_project_data_error() {
        let remote = ProjectDataProviderBuilder::default()
            .cmd_info(CmdInfo::Project(Project::default()))
            .error(true)
            .build()
            .unwrap();
        let remote = Arc::new(remote);
        let mut writer = Vec::new();
        project_info(remote, &mut writer, None).unwrap_err();
        assert!(writer.len() == 0);
    }

    #[test]
    fn test_get_project_data_wrong_cmdinfo_invariant() {
        let remote = ProjectDataProviderBuilder::default()
            .cmd_info(CmdInfo::Ignore)
            .build()
            .unwrap();
        let remote = Arc::new(remote);
        let mut writer = Vec::new();
        let result = project_info(remote, &mut writer, Some(1));
        match result {
            Ok(_) => panic!("Expected error"),
            Err(err) => match err.downcast_ref::<error::GRError>() {
                Some(error::GRError::ApplicationError(_)) => (),
                _ => panic!("Expected error::GRError::ApplicationError"),
            },
        }
    }
}
