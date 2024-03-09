use crate::Result;
/// Common functions and macros that are used by multiple commands
use std::io::Write;
use std::sync::Arc;

use crate::api_traits::Deploy;

macro_rules! query_pages {
    ($func_name:ident, $trait_name:ident) => {
        pub fn $func_name<W: Write>(remote: Arc<dyn $trait_name>, mut writer: W) -> Result<()> {
            match remote.num_pages() {
                Ok(Some(pages)) => {
                    writer.write_all(format!("{pages}\n", pages = pages).as_bytes())?
                }
                Ok(None) => {
                    writer.write_all(b"Number of pages not available.\n")?;
                }
                Err(e) => {
                    return Err(e);
                }
            };
            Ok(())
        }
    };
}

query_pages!(num_release_pages, Deploy);
