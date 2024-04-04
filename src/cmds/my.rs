use std::{io::Write, sync::Arc};

use crate::{
    api_traits::RemoteProject,
    cli::my::MyOptions,
    config::Config,
    remote::{self, ListRemoteCliArgs},
    Result,
};

use super::{
    common, merge_request,
    project::{ProjectListBodyArgs, ProjectListCliArgs},
};

pub fn execute(
    options: MyOptions,
    config: Arc<Config>,
    domain: String,
    path: String,
) -> Result<()> {
    match options {
        MyOptions::MergeRequest(cli_args) => {
            let user = get_user(&domain, &path, &config, &cli_args.list_args)?;
            merge_request::list_merge_requests(domain, path, config, cli_args, Some(user.id))
        }
        MyOptions::Project(cli_args) => {
            let user = get_user(&domain, &path, &config, &cli_args.list_args)?;
            let remote = remote::get_project(
                domain,
                path,
                config,
                cli_args.list_args.get_args.refresh_cache,
            )?;
            let from_to_args = remote::validate_from_to_page(&cli_args.list_args)?;
            list_user_projects(
                remote,
                ProjectListBodyArgs::builder()
                    .from_to_page(from_to_args)
                    .user_id(Some(user.id))
                    .build()?,
                cli_args,
                std::io::stdout(),
            )
        }
    }
}

fn get_user(
    domain: &String,
    path: &String,
    config: &Arc<Config>,
    cli_args: &ListRemoteCliArgs,
) -> Result<User> {
    let remote = remote::get_auth_user(
        domain.clone(),
        path.clone(),
        config.clone(),
        cli_args.get_args.refresh_cache,
    )?;
    let user = remote.get()?;
    Ok(user)
}

pub struct User {
    pub id: i64,
}

impl User {
    pub fn new(id: i64) -> Self {
        User { id }
    }
}

fn list_user_projects<W: Write>(
    remote: Arc<dyn RemoteProject>,
    body_args: ProjectListBodyArgs,
    cli_args: ProjectListCliArgs,
    writer: W,
) -> Result<()> {
    common::list_user_projects(remote, body_args, cli_args, writer)
}

#[cfg(test)]
mod tests {
    use crate::cmds::project::ProjectListCliArgs;

    use self::remote::{ListRemoteCliArgs, Project};

    use super::*;

    struct MockGitlab {
        projects: Vec<Project>,
    }

    impl MockGitlab {
        fn new(projects: Vec<Project>) -> Self {
            MockGitlab { projects }
        }
    }

    impl RemoteProject for MockGitlab {
        fn list(&self, _args: ProjectListBodyArgs) -> Result<Vec<Project>> {
            Ok(self.projects.clone())
        }

        fn get_project_data(&self, _id: Option<i64>) -> Result<crate::io::CmdInfo> {
            todo!()
        }

        fn get_project_members(&self) -> Result<crate::io::CmdInfo> {
            todo!()
        }

        fn get_url(&self, _option: crate::cli::browse::BrowseOptions) -> String {
            todo!()
        }
    }

    #[test]
    fn test_list_current_user_projects() {
        let projects = vec![Project::new(1, "main"), Project::new(2, "dev")];
        let user_id = 1;
        let cli_args = ProjectListCliArgs::builder()
            .list_args(ListRemoteCliArgs::builder().build().unwrap())
            .build()
            .unwrap();
        let body_args = ProjectListBodyArgs::builder()
            .from_to_page(None)
            .user_id(Some(user_id))
            .build()
            .unwrap();
        let mut buffer = Vec::new();
        assert!(list_user_projects(
            Arc::new(MockGitlab::new(projects)),
            body_args,
            cli_args,
            &mut buffer,
        )
        .is_ok());
        assert_eq!(
            "ID|Default Branch|URL\n1|main|\n2|dev|\n",
            String::from_utf8(buffer).unwrap()
        );
    }
}
