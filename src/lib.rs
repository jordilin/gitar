pub mod api_defaults;
pub mod api_traits;
pub mod browse;
pub mod cache;
pub mod cicd;
pub mod cli;
pub mod config;
pub mod dialog;
pub mod error;
pub mod exec;
pub mod git;
pub mod github;
pub mod gitlab;
pub mod http;
pub mod io;
pub mod merge_request;
pub mod remote;
pub mod shell;
pub mod test;
pub type Result<T> = anyhow::Result<T>;
pub type Cmd<T> = Box<dyn Fn() -> Result<T> + Send + Sync>;

#[macro_use]
extern crate lazy_static;
