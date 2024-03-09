/// Common functions and macros that are used by multiple commands
use crate::Result;
use std::io::Write;
use std::sync::Arc;

use crate::api_traits::{Cicd, Deploy};

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
