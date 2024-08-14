use crate::api_traits::{RemoteProject, Timestamp};
use crate::cli::project::ProjectOptions;
use crate::config::ConfigProperties;
use crate::display::{self, Column, DisplayBody};
use crate::error;
use crate::io::CmdInfo;
use crate::remote::{self, CacheType, GetRemoteCliArgs, ListBodyArgs, ListRemoteCliArgs};
use crate::Result;
use std::io::Write;
use std::sync::Arc;

#[derive(Builder, Clone, Debug, Default, PartialEq)]
pub struct Project {
    pub id: i64,
    default_branch: String,
    #[builder(default)]
    members: Vec<Member>,
    html_url: String,
    created_at: String,
    description: String,
    // Field not available in Gitlab. Set to empty string.
    #[builder(default)]
    language: String,
}

impl Project {
    pub fn builder() -> ProjectBuilder {
        ProjectBuilder::default()
    }

    pub fn new(id: i64, default_branch: &str) -> Self {
        Project {
            id,
            default_branch: default_branch.to_string(),
            members: Vec::new(),
            html_url: String::new(),
            created_at: String::new(),
            description: String::new(),
            language: String::new(),
        }
    }

    pub fn with_html_url(mut self, html_url: &str) -> Self {
        self.html_url = html_url.to_string();
        self
    }

    // TODO - builder pattern
    pub fn with_created_at(mut self, created_at: &str) -> Self {
        self.created_at = created_at.to_string();
        self
    }

    pub fn default_branch(&self) -> &str {
        &self.default_branch
    }
}

impl From<Project> for DisplayBody {
    fn from(p: Project) -> DisplayBody {
        DisplayBody {
            columns: vec![
                Column::new("ID", p.id.to_string()),
                Column::new("Default Branch", p.default_branch),
                Column::new("URL", p.html_url),
                Column::new("Created at", p.created_at),
                Column::builder()
                    .name("Description".to_string())
                    .value(p.description)
                    .optional(true)
                    .build()
                    .unwrap(),
                Column::builder()
                    .name("Language".to_string())
                    .value(p.language)
                    .optional(true)
                    .build()
                    .unwrap(),
            ],
        }
    }
}

impl Timestamp for Project {
    fn created_at(&self) -> String {
        self.created_at.clone()
    }
}

#[derive(Builder, Clone, Debug, PartialEq, Default)]
pub struct Member {
    pub id: i64,
    pub name: String,
    pub username: String,
    #[builder(default)]
    pub created_at: String,
}

impl Member {
    pub fn builder() -> MemberBuilder {
        MemberBuilder::default()
    }
}

impl Timestamp for Member {
    fn created_at(&self) -> String {
        self.created_at.clone()
    }
}

impl From<Member> for DisplayBody {
    fn from(m: Member) -> DisplayBody {
        DisplayBody {
            columns: vec![
                Column::new("ID", m.id.to_string()),
                Column::new("Name", m.name),
                Column::new("Username", m.username),
            ],
        }
    }
}

#[derive(Builder)]
pub struct ProjectListCliArgs {
    pub list_args: ListRemoteCliArgs,
    #[builder(default)]
    pub stars: bool,
    #[builder(default)]
    pub tags: bool,
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
    config: Arc<dyn ConfigProperties>,
    domain: String,
    path: String,
) -> Result<()> {
    match options {
        ProjectOptions::Info(cli_args) => {
            let remote = remote::get_project(
                domain,
                path,
                config,
                Some(&cli_args.get_args.cache_args),
                CacheType::File,
            )?;
            project_info(remote, std::io::stdout(), cli_args)
        }
        _ => todo!(),
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
    use crate::cli::browse::BrowseOptions;

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
