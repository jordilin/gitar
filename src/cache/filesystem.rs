use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use flate2::bufread::GzDecoder;
use sha2::{Digest, Sha256};

use crate::cache::Cache;
use crate::http::{Headers, Resource};
use crate::io::{self, FlowControlHeaders, HttpResponse};
use crate::time::Seconds;

use super::CacheState;

use crate::config::ConfigProperties;

use crate::error::{self, AddContext, GRError};
use crate::Result;

use flate2::write::GzEncoder;
use flate2::Compression;

pub struct FileCache {
    config: Arc<dyn ConfigProperties>,
}

impl FileCache {
    pub fn new(config: Arc<dyn ConfigProperties>) -> Self {
        FileCache { config }
    }

    pub fn validate_cache_location(&self) -> Result<()> {
        let cache_location = self
            .config
            .cache_location()
            .ok_or(GRError::ConfigurationNotFound)?;

        let path = Path::new(cache_location);

        if !path.exists() {
            return Err(GRError::CacheLocationDoesNotExist(format!(
                "Cache directory does not exist: {cache_location}"
            ))
            .into());
        }

        if !path.is_dir() {
            return Err(GRError::CacheLocationIsNotADirectory(format!(
                "Cache location is not a directory: {cache_location}"
            ))
            .into());
        }

        // Check if we can write to the directory
        let test_file_path = path.join(".write_test_cache_file");
        match File::create(&test_file_path) {
            Ok(_) => {
                // Successfully created the file, now remove it
                if let Err(e) = fs::remove_file(&test_file_path) {
                    return Err(GRError::CacheLocationWriteTestFailed(format!(
                        "Failed to remove cache test file {}: {}",
                        test_file_path.to_string_lossy(),
                        e
                    ))
                    .into());
                }
            }
            Err(e) => {
                return Err(GRError::CacheLocationIsNotWriteable(format!(
                    "No write permission for cache directory {cache_location}: {e}"
                ))
                .into());
            }
        }
        Ok(())
    }

    pub fn get_cache_file(&self, url: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(url);
        let hash = hasher.finalize();
        let cache_location = self.config.cache_location().unwrap();
        let location = cache_location.strip_suffix('/').unwrap_or(cache_location);
        format!("{location}/{hash:x}")
    }

    fn get_cache_data(&self, mut reader: impl BufRead) -> Result<HttpResponse> {
        let decompressed_data = GzDecoder::new(&mut reader);
        let mut reader = BufReader::new(decompressed_data);
        let mut headers = String::new();
        reader.read_line(&mut headers)?;
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
                    "Could not parse the response status code from cache {err}"
                );
                return Err(error::gen(trace));
            }
        };
        let mut body = Vec::new();
        reader.read_to_end(&mut body)?;
        let body = String::from_utf8(body)?.trim().to_string();
        let headers_map = serde_json::from_str::<Headers>(&headers)?;
        // Gather cached link headers for pagination.
        // We don't need rate limit headers as we are not querying the API at
        // this point.
        let page_header = io::parse_page_headers(Some(&headers_map));
        let flow_control_headers = FlowControlHeaders::new(Rc::new(page_header), Rc::new(None));

        let response = HttpResponse::builder()
            .status(status_code)
            .body(body)
            .headers(headers_map)
            .flow_control_headers(flow_control_headers)
            .build()?;
        Ok(response)
    }

    fn persist_cache_data(&self, value: &HttpResponse, f: BufWriter<File>) -> Result<()> {
        let headers_map = value.headers.as_ref().unwrap();
        let headers = serde_json::to_string(headers_map).unwrap();
        let status = value.status.to_string();
        let file_data = format!("{}\n{}\n{}", headers, status, value.body);
        let mut encoder = GzEncoder::new(f, Compression::default());
        encoder.write_all(file_data.as_bytes())?;
        Ok(())
    }

    fn expired(
        &self,
        key: &Resource,
        path: String,
        cache_control: Option<CacheControl>,
    ) -> Result<bool> {
        let cache_expiration = self
            .config
            .get_cache_expiration(key.api_operation.as_ref().unwrap())
            .try_into()
            .err_context(GRError::ConfigurationError(format!(
                "Cannot retrieve cache expiration time. \
                 Check your configuration file and make sure the key \
                 <domain>.cache_api_{}_expiration has a valid time format.",
                &key.api_operation.as_ref().unwrap()
            )))?;
        expired(
            || get_file_mtime_elapsed(path.as_str()),
            cache_expiration,
            cache_control,
        )
    }
}

impl Cache<Resource> for FileCache {
    fn get(&self, key: &Resource) -> Result<CacheState> {
        let path = self.get_cache_file(&key.url);
        if let Ok(f) = File::open(&path) {
            let mut f = BufReader::new(f);
            let response = self.get_cache_data(&mut f)?;

            let cache_control = response.headers.as_ref().and_then(parse_cache_control);

            if self.expired(key, path, cache_control)? {
                return Ok(CacheState::Stale(response));
            }
            Ok(CacheState::Fresh(response))
        } else {
            Ok(CacheState::None)
        }
    }

    fn set(&self, key: &Resource, value: &HttpResponse) -> Result<()> {
        let path = self.get_cache_file(&key.url);
        let f = File::create(path)?;
        let f = BufWriter::new(f);
        self.persist_cache_data(value, f)?;
        Ok(())
    }

    fn update(
        &self,
        key: &Resource,
        value: &HttpResponse,
        field: &io::ResponseField,
    ) -> Result<()> {
        let path = self.get_cache_file(&key.url);
        if let Ok(f) = File::open(path) {
            let mut f = BufReader::new(f);
            let mut response = self.get_cache_data(&mut f)?;
            match field {
                io::ResponseField::Body => response.body.clone_from(&value.body),
                io::ResponseField::Headers => {
                    // update existing headers with new ones. Not guaranteed
                    // that a 304 will actually contain *all* the headers that
                    // we got from an original 200 response. Update existing and
                    // maintain old ones. Github wipes link headers on 304s that
                    // actually existed in 200s.
                    response
                        .headers
                        .as_mut()
                        .unwrap()
                        .extend(value.headers.as_ref().unwrap().clone());
                }
                io::ResponseField::Status => response.status = value.status,
            }
            return self.set(key, &response);
        }
        Ok(())
    }
}

struct CacheControl {
    max_age: Option<Seconds>,
    no_cache: bool,
    no_store: bool,
}

fn parse_cache_control(headers: &Headers) -> Option<CacheControl> {
    headers.get("cache-control").map(|cc| {
        let mut max_age = None;
        let mut no_cache = false;
        let mut no_store = false;

        for directive in cc.split(',') {
            let directive = directive.trim().to_lowercase();
            if directive == "no-cache" {
                no_cache = true;
            } else if directive == "no-store" {
                no_store = true;
            } else if let Some(exp) = directive.strip_prefix("max-age=") {
                max_age = exp.parse().ok();
            }
        }

        CacheControl {
            max_age,
            no_cache,
            no_store,
        }
    })
}

fn expired<F: Fn() -> Result<Seconds>>(
    get_file_mtime_elapsed: F,
    refresh_every: Seconds,
    cache_control: Option<CacheControl>,
) -> Result<bool> {
    let elapsed = get_file_mtime_elapsed()?;

    // Check user-defined expiration first
    if elapsed < refresh_every {
        return Ok(false);
    }

    // If user-defined expiration is reached, then consider cache-control
    if let Some(cc) = cache_control {
        if cc.no_store {
            return Ok(true);
        }
        if cc.no_cache {
            return Ok(true);
        }
        if let Some(max_age) = cc.max_age {
            return Ok(elapsed >= max_age);
        }
    }

    // If no cache-control or no relevant directives, it's expired
    Ok(true)
}

fn get_file_mtime_elapsed(path: &str) -> Result<Seconds> {
    let metadata = std::fs::metadata(path)?;
    let mtime = metadata.modified()?.elapsed()?.as_secs();
    Ok(Seconds::new(mtime))
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
        fn cache_location(&self) -> Option<&str> {
            // TODO test with suffix /
            // should probably be sanitized on the Config struct itself.
            Some("/home/user/.cache")
        }
    }

    #[test]
    fn test_get_cache_file() {
        let config = ConfigMock::new();
        let file_cache = FileCache::new(Arc::new(config));
        let url = "https://gitlab.org/api/v4/projects/jordilin%2Fmr";
        let cache_file = file_cache.get_cache_file(url);
        assert_eq!(
            "/home/user/.cache/b677b4f27bfd83c168c62cb1b629ac06e9444c29c0380a20ea2f2cad266f7dd9",
            cache_file
        );
    }

    #[test]
    fn test_get_cache_data() {
        let cached_data = r#"{"vary":"Accept-Encoding","cache-control":"max-age=0, private, must-revalidate","server":"nginx","transfer-encoding":"chunked","x-content-type-options":"nosniff","etag":"W/\"9ef5b79701ae0a753b6f08dc9229cdb6\"","x-per-page":"20","date":"Sat, 13 Jan 2024 19:50:23 GMT","connection":"keep-alive","x-next-page":"","x-runtime":"0.050489","content-type":"application/json","x-total-pages":"2","strict-transport-security":"max-age=63072000","referrer-policy":"strict-origin-when-cross-origin","x-prev-page":"1","x-request-id":"01HM260622PFEYAHAZQQWNT1WG","x-total":"22","x-page":"2","link":"<http://gitlab-web/api/v4/projects/tooling%2Fcli/members/all?id=tooling%2Fcli&page=1&per_page=20>; rel=\"prev\", <http://gitlab-web/api/v4/projects/tooling%2Fcli/members/all?id=tooling%2Fcli&page=1&per_page=20>; rel=\"first\", <http://gitlab-web/api/v4/projects/tooling%2Fcli/members/all?id=tooling%2Fcli&page=2&per_page=20>; rel=\"last\"","x-frame-options":"SAMEORIGIN"}
        200
        {"name":"385db2892449a18ca075c40344e6e9b418e3b16c","path":"tooling/cli:385db2892449a18ca075c40344e6e9b418e3b16c","location":"localhost:4567/tooling/cli:385db2892449a18ca075c40344e6e9b418e3b16c","revision":"791d4b6a13f90f0e48dd68fa1c758b79a6936f3854139eb01c9f251eded7c98d","short_revision":"791d4b6a1","digest":"sha256:41c70f2fcb036dfc6ca7da19b25cb660055268221b9d5db666bdbc7ad1ca2029","created_at":"2022-06-29T15:56:01.580+00:00","total_size":2819312
        "#;
        let mut enc = GzEncoder::new(Vec::new(), Compression::default());
        enc.write_all(cached_data.as_bytes()).unwrap();
        let reader = std::io::Cursor::new(enc.finish().unwrap());
        let fc = FileCache::new(Arc::new(ConfigMock::new()));
        let response = fc.get_cache_data(reader).unwrap();

        assert_eq!(200, response.status);
        assert_eq!(
                    "<http://gitlab-web/api/v4/projects/tooling%2Fcli/members/all?id=tooling%2Fcli&page=1&per_page=20>; rel=\"prev\", <http://gitlab-web/api/v4/projects/tooling%2Fcli/members/all?id=tooling%2Fcli&page=1&per_page=20>; rel=\"first\", <http://gitlab-web/api/v4/projects/tooling%2Fcli/members/all?id=tooling%2Fcli&page=2&per_page=20>; rel=\"last\"",
                    response.headers.as_ref().unwrap().get(io::LINK_HEADER).unwrap()
                );
        assert_eq!(
                    "{\"name\":\"385db2892449a18ca075c40344e6e9b418e3b16c\",\"path\":\"tooling/cli:385db2892449a18ca075c40344e6e9b418e3b16c\",\"location\":\"localhost:4567/tooling/cli:385db2892449a18ca075c40344e6e9b418e3b16c\",\"revision\":\"791d4b6a13f90f0e48dd68fa1c758b79a6936f3854139eb01c9f251eded7c98d\",\"short_revision\":\"791d4b6a1\",\"digest\":\"sha256:41c70f2fcb036dfc6ca7da19b25cb660055268221b9d5db666bdbc7ad1ca2029\",\"created_at\":\"2022-06-29T15:56:01.580+00:00\",\"total_size\":2819312",
                    response.body
                );
    }

    fn mock_file_mtime_elapsed(m_time: u64) -> Result<Seconds> {
        Ok(Seconds::new(m_time))
    }

    #[test]
    fn test_expired_cache_beyond_refresh_time() {
        assert!(expired(|| mock_file_mtime_elapsed(500), Seconds::new(300), None).unwrap())
    }

    #[test]
    fn test_expired_diff_now_and_cache_same_as_refresh() {
        assert!(expired(|| mock_file_mtime_elapsed(300), Seconds::new(300), None).unwrap())
    }

    #[test]
    fn test_not_expired_diff_now_and_cache_less_than_refresh() {
        assert!(!expired(|| mock_file_mtime_elapsed(100), Seconds::new(1000), None).unwrap())
    }

    #[test]
    fn test_expired_get_m_time_result_err() {
        assert!(expired(
            || Err(error::gen("Could not get file mtime")),
            Seconds::new(1000),
            None
        )
        .is_err())
    }

    fn cc(max_age: Option<Seconds>, no_cache: bool, no_store: bool) -> Option<CacheControl> {
        Some(CacheControl {
            max_age,
            no_cache,
            no_store,
        })
    }

    #[test]
    fn test_cache_not_expired_according_to_user_cache_control_ignored() {
        let user_refresh = Seconds::new(3600);

        assert!(!expired(
            || Ok(Seconds::new(3000)),
            user_refresh,
            cc(Some(Seconds::new(2000)), false, false)
        )
        .unwrap());
    }

    #[test]
    fn test_cache_expired_according_to_user_checks_http_cache_control() {
        let user_refresh = Seconds::new(3600);

        assert!(!expired(
            || Ok(Seconds::new(3601)),
            user_refresh,
            cc(Some(Seconds::new(4000)), false, false)
        )
        .unwrap());

        assert!(expired(
            || Ok(Seconds::new(4001)),
            user_refresh,
            cc(Some(Seconds::new(4000)), false, false)
        )
        .unwrap());
    }

    #[test]
    fn test_cache_expired_according_to_user_no_cache_control_directive() {
        let user_refresh = Seconds::new(3600);

        assert!(expired(
            || Ok(Seconds::new(3601)),
            user_refresh,
            cc(None, true, false)
        )
        .unwrap());
    }

    #[test]
    fn test_cache_expired_according_to_user_no_store_directive() {
        let user_refresh = Seconds::new(3600);

        assert!(expired(
            || Ok(Seconds::new(3601)),
            user_refresh,
            cc(None, false, true)
        )
        .unwrap());
    }

    #[test]
    fn test_cache_expired_according_to_user_no_cache_control_whatsoever() {
        let user_refresh = Seconds::new(3600);
        assert!(expired(|| Ok(Seconds::new(3601)), user_refresh, None).unwrap());
    }

    #[test]
    fn test_user_expires_cache_but_http_cache_control_not_expired() {
        let user_refresh = Seconds::new(3600);

        assert!(!expired(
            || Ok(Seconds::new(5000)),
            user_refresh,
            cc(Some(Seconds::new(6000)), false, false)
        )
        .unwrap());
    }

    #[test]
    fn test_cache_expired_both_user_and_cache_control() {
        let user_refresh = Seconds::new(3600);

        assert!(expired(
            || Ok(Seconds::new(7000)),
            user_refresh,
            cc(Some(Seconds::new(6000)), false, false)
        )
        .unwrap());
    }

    #[test]
    fn test_parse_cache_control() {
        let mut headers = Headers::new();

        let test_table = vec![
            (
                "max-age=3600, no-cache, no-store",
                Some(Seconds::new(3600)),
                true,
                true,
            ),
            (
                "max-age=3600, no-cache",
                Some(Seconds::new(3600)),
                true,
                false,
            ),
            (
                "max-age=3600, no-store",
                Some(Seconds::new(3600)),
                false,
                true,
            ),
            ("no-cache, no-store", None, true, true),
            ("no-cache", None, true, false),
            ("no-store", None, false, true),
            ("max-age=0", Some(Seconds::new(0)), false, false),
        ];

        for (header, max_age, no_cache, no_store) in test_table {
            headers.set("cache-control".to_string(), header.to_string());
            let cc = parse_cache_control(&headers).unwrap();
            assert_eq!(cc.max_age, max_age);
            assert_eq!(cc.no_cache, no_cache);
            assert_eq!(cc.no_store, no_store);
        }
    }
}
