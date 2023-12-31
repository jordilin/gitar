use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};

use sha2::{Digest, Sha256};

use crate::cache::Cache;
use crate::io::{self, Response};

use super::CacheState;

use crate::config::ConfigProperties;

use crate::error;
use crate::Result;

pub struct FileCache<C> {
    config: C,
}

impl<C: ConfigProperties> FileCache<C> {
    pub fn new(config: C) -> Self {
        FileCache { config }
    }

    fn get_cache_file(&self, url: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(url);
        let hash = hasher.finalize();
        let cache_location = self.config.cache_location();
        let location = cache_location.strip_suffix('/').unwrap_or(cache_location);
        format!("{}/{:x}", location, hash)
    }

    fn get_cache_data(&self, mut reader: impl BufRead) -> Result<CacheState> {
        let mut link_header = String::new();
        reader.read_line(&mut link_header)?;
        let link_header = link_header.trim();
        let mut status_code = String::new();
        reader.read_line(&mut status_code)?;
        let status_code = status_code.trim();
        let status_code = match status_code.parse::<i32>() {
            Ok(value) => value,
            Err(err) => {
                // parse error in here could be hard to find/debug. Send a clear
                // error trace over to the client.
                // TODO should we just treat it as a cache miss?
                let trace = format!(
                    "Could not parse the response status code from cache {}",
                    err
                );
                return Err(error::gen(trace));
            }
        };
        let mut body = String::new();
        reader.read_line(&mut body)?;
        let body = body.trim();
        let mut headers = HashMap::new();
        headers.insert(io::LINK_HEADER.to_string(), link_header.to_string());
        let response = io::Response::new()
            .with_body(body.to_string())
            .with_headers(headers)
            .with_status(status_code);
        Ok(CacheState::Fresh(response))
    }

    fn persist_cache_data(&self, value: &Response, mut f: BufWriter<File>) -> Result<()> {
        let default_link_header = String::from("");
        let links = value
            .headers
            .as_ref()
            .unwrap()
            .get(io::LINK_HEADER)
            .unwrap_or(&default_link_header);
        f.write_all(links.as_bytes())?;
        f.write_all(b"\n")?;
        f.write_all(value.status.to_string().as_bytes())?;
        f.write_all(b"\n")?;
        f.write_all(value.body.as_bytes())?;
        Ok(())
    }
}

impl<C: ConfigProperties> Cache for FileCache<C> {
    fn get(&self, key: &str) -> Result<CacheState> {
        let path = self.get_cache_file(key);
        if let Ok(f) = File::open(path) {
            let mut f = BufReader::new(f);
            self.get_cache_data(&mut f)
        } else {
            Ok(CacheState::None)
        }
    }

    fn set(&self, key: &str, value: &Response) -> Result<()> {
        let path = self.get_cache_file(key);
        let f = File::create(path)?;
        let f = BufWriter::new(f);
        self.persist_cache_data(value, f)?;
        Ok(())
    }
}

// test
#[cfg(test)]
mod tests {
    use super::*;

    struct ConfigMock;

    impl ConfigMock {
        fn new() -> Self {
            ConfigMock {}
        }
    }

    impl ConfigProperties for ConfigMock {
        fn api_token(&self) -> &str {
            "1234"
        }
        fn cache_location(&self) -> &str {
            // TODO test with suffix /
            // should probably be sanitized on the Config struct itself.
            "/home/user/.cache"
        }
    }

    #[test]
    fn test_get_cache_file() {
        let config = ConfigMock::new();
        let file_cache = FileCache::new(config);
        let url = "https://gitlab.org/api/v4/projects/jordilin%2Fmr";
        let cache_file = file_cache.get_cache_file(url);
        assert_eq!(
            "/home/user/.cache/b677b4f27bfd83c168c62cb1b629ac06e9444c29c0380a20ea2f2cad266f7dd9",
            cache_file
        );
    }

    #[test]
    fn test_get_cache_data() {
        let cached_data = r#"<https://gitlab.com/api/v4/projects/jordilin%2Fmr/merge_requests?per_page=100&page=2>; rel="next", <https://gitlab.com/api/v4/projects/jordilin%2Fmr/merge_requests?per_page=100&page=1>; rel="first", <https://gitlab.com/api/v4/projects/jordilin%2Fmr/merge_requests?per_page=100&page=2>; rel="last"
        200
        {"name":"385db2892449a18ca075c40344e6e9b418e3b16c","path":"tooling/cli:385db2892449a18ca075c40344e6e9b418e3b16c","location":"localhost:4567/tooling/cli:385db2892449a18ca075c40344e6e9b418e3b16c","revision":"791d4b6a13f90f0e48dd68fa1c758b79a6936f3854139eb01c9f251eded7c98d","short_revision":"791d4b6a1","digest":"sha256:41c70f2fcb036dfc6ca7da19b25cb660055268221b9d5db666bdbc7ad1ca2029","created_at":"2022-06-29T15:56:01.580+00:00","total_size":2819312
        "#;
        let reader = std::io::Cursor::new(cached_data);
        let fc = FileCache::new(ConfigMock::new());
        let cache_data = fc.get_cache_data(reader).unwrap();
        match cache_data {
            CacheState::Fresh(response) => {
                assert_eq!(200, response.status);
                assert_eq!(
                    "<https://gitlab.com/api/v4/projects/jordilin%2Fmr/merge_requests?per_page=100&page=2>; rel=\"next\", <https://gitlab.com/api/v4/projects/jordilin%2Fmr/merge_requests?per_page=100&page=1>; rel=\"first\", <https://gitlab.com/api/v4/projects/jordilin%2Fmr/merge_requests?per_page=100&page=2>; rel=\"last\"",
                    response.headers.as_ref().unwrap().get(io::LINK_HEADER).unwrap()
                );
                assert_eq!(
                    "{\"name\":\"385db2892449a18ca075c40344e6e9b418e3b16c\",\"path\":\"tooling/cli:385db2892449a18ca075c40344e6e9b418e3b16c\",\"location\":\"localhost:4567/tooling/cli:385db2892449a18ca075c40344e6e9b418e3b16c\",\"revision\":\"791d4b6a13f90f0e48dd68fa1c758b79a6936f3854139eb01c9f251eded7c98d\",\"short_revision\":\"791d4b6a1\",\"digest\":\"sha256:41c70f2fcb036dfc6ca7da19b25cb660055268221b9d5db666bdbc7ad1ca2029\",\"created_at\":\"2022-06-29T15:56:01.580+00:00\",\"total_size\":2819312",
                    response.body
                );
            }
            _ => panic!("Expected a fresh cache state"),
        }
    }
}
