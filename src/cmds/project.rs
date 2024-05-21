use crate::api_traits::RemoteProject;
use crate::cli::project::ProjectOptions;
use crate::config::Config;
use crate::display;
use crate::error;
use crate::io::CmdInfo;
use crate::remote::ListBodyArgs;
use crate::remote::ListRemoteCliArgs;
use crate::remote::Member;
use crate::remote::{self, GetRemoteCliArgs};
use crate::Result;
use std::io::Write;
use std::sync::Arc;

#[derive(Builder)]
pub struct ProjectListCliArgs {
    pub list_args: ListRemoteCliArgs,
    #[builder(default)]
    pub stars: bool,
}

impl ProjectListCliArgs {
    pub fn builder() -> ProjectListCliArgsBuilder {
        ProjectListCliArgsBuilder::default()
    }
}

#[derive(Builder)]
pub struct ProjectListBodyArgs {
    pub from_to_page: Option<ListBodyArgs>,
    pub user: Option<Member>,
    #[builder(default)]
    pub stars: bool,
}

impl ProjectListBodyArgs {
    pub fn builder() -> ProjectListBodyArgsBuilder {
        ProjectListBodyArgsBuilder::default()
    }
}

#[derive(Builder)]
pub struct ProjectMetadataGetCliArgs {
    pub id: Option<i64>,
    #[builder(default)]
    pub path: Option<String>,
    pub get_args: GetRemoteCliArgs,
}

impl ProjectMetadataGetCliArgs {
    pub fn builder() -> ProjectMetadataGetCliArgsBuilder {
        ProjectMetadataGetCliArgsBuilder::default()
    }
}

pub fn execute(
    options: ProjectOptions,
    config: Arc<Config>,
    domain: String,
    path: String,
) -> Result<()> {
    match options {
        ProjectOptions::Info(cli_args) => {
            let remote =
                remote::get_project(domain, path, config, cli_args.get_args.refresh_cache)?;
            project_info(remote, std::io::stdout(), cli_args)
        }
    }
}

fn project_info<W: Write>(
    remote: Arc<dyn RemoteProject>,
    mut writer: W,
    cli_args: ProjectMetadataGetCliArgs,
) -> Result<()> {
    let CmdInfo::Project(project_data) =
        remote.get_project_data(cli_args.id, cli_args.path.as_deref())?
    else {
        return Err(error::GRError::ApplicationError(
            "remote.get_project_data expects CmdInfo::Project invariant".to_string(),
        )
        .into());
    };
    display::print(&mut writer, vec![project_data], cli_args.get_args)?;
    Ok(())
}

#[cfg(test)]
mod test {

    use std::cell::RefCell;

    use super::*;
    use crate::{cli::browse::BrowseOptions, remote::Project};

    #[derive(Builder)]
    struct ProjectDataProvider {
        #[builder(default = "false")]
        error: bool,
        cmd_info: CmdInfo,
        #[builder(default = "RefCell::new(false)")]
        project_data_with_id_called: RefCell<bool>,
        #[builder(default = "RefCell::new(false)")]
        project_data_with_path_called: RefCell<bool>,
    }

    impl RemoteProject for ProjectDataProvider {
        fn get_project_data(&self, id: Option<i64>, path: Option<&str>) -> crate::Result<CmdInfo> {
            if let Some(_) = id {
                *self.project_data_with_id_called.borrow_mut() = true;
            }
            if let Some(_) = path {
                *self.project_data_with_path_called.borrow_mut() = true;
            }
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

        fn get_url(&self, _option: BrowseOptions) -> String {
            todo!()
        }

        fn list(&self, _args: ProjectListBodyArgs) -> Result<Vec<Project>> {
            todo!()
        }

        fn num_pages(&self, _args: ProjectListBodyArgs) -> Result<Option<u32>> {
            todo!()
        }

        fn num_resources(
            &self,
            _args: ProjectListBodyArgs,
        ) -> Result<Option<crate::api_traits::NumberDeltaErr>> {
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
        let get_args = GetRemoteCliArgs::default();
        let cli_args = ProjectMetadataGetCliArgs::builder()
            .id(Some(1))
            .get_args(get_args)
            .build()
            .unwrap();
        project_info(remote.clone(), &mut writer, cli_args).unwrap();
        assert!(writer.len() > 0);
        assert!(*remote.project_data_with_id_called.borrow());
    }

    #[test]
    fn test_project_data_called_by_repo_path() {
        let remote = ProjectDataProviderBuilder::default()
            .cmd_info(CmdInfo::Project(Project::default()))
            .build()
            .unwrap();
        let remote = Arc::new(remote);
        let mut writer = Vec::new();
        let get_args = GetRemoteCliArgs::default();
        let cli_args = ProjectMetadataGetCliArgs::builder()
            .id(None)
            .path(Some("jordilin/gitar".to_string()))
            .get_args(get_args)
            .build()
            .unwrap();
        project_info(remote.clone(), &mut writer, cli_args).unwrap();
        assert!(writer.len() > 0);
        assert!(*remote.project_data_with_path_called.borrow());
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
        let get_args = GetRemoteCliArgs::default();
        let cli_args = ProjectMetadataGetCliArgs::builder()
            .id(Some(1))
            .get_args(get_args)
            .build()
            .unwrap();
        project_info(remote, &mut writer, cli_args).unwrap_err();
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
        let get_args = GetRemoteCliArgs::default();
        let cli_args = ProjectMetadataGetCliArgs::builder()
            .id(Some(1))
            .get_args(get_args)
            .build()
            .unwrap();
        let result = project_info(remote, &mut writer, cli_args);
        match result {
            Ok(_) => panic!("Expected error"),
            Err(err) => match err.downcast_ref::<error::GRError>() {
                Some(error::GRError::ApplicationError(_)) => (),
                _ => panic!("Expected error::GRError::ApplicationError"),
            },
        }
    }
}
