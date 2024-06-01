pub mod api_defaults;
pub mod api_traits;
pub mod cache;
pub mod cli;
pub mod config;
pub mod dialog;
pub mod error;
pub mod exec;
pub mod git;
pub mod github;
pub mod gitlab;
pub mod http;
pub mod init;
pub mod io;
pub mod remote;
pub mod shell;
pub mod test;
pub mod time;
pub type Result<T> = anyhow::Result<T>;
pub type Error = anyhow::Error;
pub type Cmd<T> = Box<dyn FnOnce() -> Result<T> + Send + Sync>;
pub mod backoff;
pub mod cmds;
pub mod display;
pub mod logging;

#[macro_use]
extern crate log;

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate derive_builder;

fn json_load_page(data: &str) -> Result<Vec<serde_json::Value>> {
    serde_json::from_str(data).map_err(|e| error::gen(e.to_string()))
}

fn json_loads(data: &str) -> Result<serde_json::Value> {
    serde_json::from_str(data).map_err(|e| error::gen(e.to_string()))
}

pub const USER_GUIDE_URL: &str = "https://jordilin.github.io/gitar";
