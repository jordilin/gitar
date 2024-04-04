use crate::display;
use crate::remote::MergeRequestListBodyArgs;
/// Common functions and macros that are used by multiple commands
use crate::Result;
use crate::{api_traits::MergeRequest, remote::ListRemoteCliArgs};
use std::io::Write;
use std::sync::Arc;

use crate::api_traits::{Cicd, CicdRunner, Deploy, RemoteProject};

use super::cicd::{RunnerListBodyArgs, RunnerListCliArgs};
use super::project::{ProjectListBodyArgs, ProjectListCliArgs};
use super::release::ReleaseBodyArgs;
use super::{cicd::PipelineBodyArgs, merge_request::MergeRequestListCliArgs};

macro_rules! query_pages {
    ($func_name:ident, $trait_name:ident) => {
        pub fn $func_name<W: Write>(remote: Arc<dyn $trait_name>, mut writer: W) -> Result<()> {
            process_num_pages(remote.num_pages(), &mut writer)
        }
    };
}

pub fn process_num_pages<W: Write>(num_pages: Result<Option<u32>>, mut writer: W) -> Result<()> {
    match num_pages {
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

query_pages!(num_release_pages, Deploy);
query_pages!(num_cicd_pages, Cicd);

macro_rules! list_resource {
    ($func_name:ident, $trait_name:ident, $body_args:ident, $cli_args:ident, $embeds_list_args: literal) => {
        pub fn $func_name<W: Write>(
            remote: Arc<dyn $trait_name>,
            body_args: $body_args,
            cli_args: $cli_args,
            mut writer: W,
        ) -> Result<()> {
            let objs = remote.list(body_args)?;
            if cli_args.list_args.flush {
                return Ok(());
            }
            if objs.is_empty() {
                writer.write_all(b"No resources found.\n")?;
                return Ok(());
            }
            display::print(&mut writer, objs, cli_args.list_args.get_args)?;
            Ok(())
        }
    };
    ($func_name:ident, $trait_name:ident, $body_args:ident, $cli_args:ident) => {
        pub fn $func_name<W: Write>(
            remote: Arc<dyn $trait_name>,
            body_args: $body_args,
            cli_args: $cli_args,
            mut writer: W,
        ) -> Result<()> {
            let objs = remote.list(body_args)?;
            if cli_args.flush {
                return Ok(());
            }
            if objs.is_empty() {
                writer.write_all(b"No resources found.\n")?;
                return Ok(());
            }
            display::print(&mut writer, objs, cli_args.get_args)?;
            Ok(())
        }
    };
}

list_resource!(
    list_merge_requests,
    MergeRequest,
    MergeRequestListBodyArgs,
    MergeRequestListCliArgs,
    true
);

list_resource!(list_pipelines, Cicd, PipelineBodyArgs, ListRemoteCliArgs);
list_resource!(
    list_runners,
    CicdRunner,
    RunnerListBodyArgs,
    RunnerListCliArgs,
    true
);

list_resource!(list_releases, Deploy, ReleaseBodyArgs, ListRemoteCliArgs);

list_resource!(
    list_user_projects,
    RemoteProject,
    ProjectListBodyArgs,
    ProjectListCliArgs,
    true
);
