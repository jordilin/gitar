#[cfg(test)]
pub mod utils {
    use crate::{
        api_defaults::REST_API_MAX_PAGES,
        api_traits::ApiOperation,
        config::ConfigProperties,
        error,
        http::{self, Headers, Request},
        io::{HttpRunner, Response, TaskRunner},
        time::Milliseconds,
        Result,
    };
    use lazy_static::lazy_static;
    use log::{Level, LevelFilter, Metadata, Record};
    use serde::Serialize;
    use std::{
        cell::{Ref, RefCell},
        fmt::Write,
        fs::File,
        io::Read,
        sync::{Arc, Mutex},
    };

    pub enum ContractType {
        Gitlab,
        Github,
        Git,
    }

    impl ContractType {
        fn as_str(&self) -> &str {
            match *self {
                ContractType::Gitlab => "gitlab",
                ContractType::Github => "github",
                ContractType::Git => "git",
            }
        }
    }

    pub fn get_contract(contract_type: ContractType, filename: &str) -> String {
        let contracts_path = format!("contracts/{}/{}", contract_type.as_str(), filename);
        let mut file = File::open(contracts_path).unwrap();
        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();
        contents
    }

    pub struct MockRunner {
        responses: RefCell<Vec<Response>>,
        cmd: RefCell<String>,
        headers: RefCell<Headers>,
        url: RefCell<String>,
        pub api_operation: RefCell<Option<ApiOperation>>,
        pub config: ConfigMock,
        pub http_method: RefCell<http::Method>,
        pub throttled: RefCell<u32>,
        pub milliseconds_throttled: RefCell<Milliseconds>,
    }

    impl MockRunner {
        pub fn new(responses: Vec<Response>) -> Self {
            Self {
                responses: RefCell::new(responses),
                cmd: RefCell::new(String::new()),
                headers: RefCell::new(Headers::new()),
                url: RefCell::new(String::new()),
                api_operation: RefCell::new(None),
                config: ConfigMock::default(),
                http_method: RefCell::new(http::Method::GET),
                throttled: RefCell::new(0),
                milliseconds_throttled: RefCell::new(Milliseconds::new(0)),
            }
        }

        pub fn with_config(self, config: ConfigMock) -> Self {
            Self { config, ..self }
        }

        pub fn cmd(&self) -> Ref<String> {
            self.cmd.borrow()
        }

        pub fn url(&self) -> Ref<String> {
            self.url.borrow()
        }

        pub fn headers(&self) -> Ref<Headers> {
            self.headers.borrow()
        }

        pub fn throttled(&self) -> Ref<u32> {
            self.throttled.borrow()
        }

        pub fn milliseconds_throttled(&self) -> Ref<Milliseconds> {
            self.milliseconds_throttled.borrow()
        }
    }

    impl TaskRunner for MockRunner {
        type Response = Response;

        fn run<T>(&self, cmd: T) -> Result<Self::Response>
        where
            T: IntoIterator,
            T::Item: AsRef<std::ffi::OsStr>,
        {
            self.cmd.replace(
                cmd.into_iter()
                    .map(|s| s.as_ref().to_str().unwrap().to_string())
                    .collect::<Vec<String>>()
                    .join(" "),
            );
            let response = self.responses.borrow_mut().pop().unwrap();
            match response.status {
                0 => return Ok(response),
                _ => return Err(error::gen(&response.body)),
            }
        }
    }

    impl HttpRunner for MockRunner {
        type Response = Response;

        fn run<T: Serialize>(&self, cmd: &mut Request<T>) -> Result<Self::Response> {
            self.url.replace(cmd.url().to_string());
            self.headers.replace(cmd.headers().clone());
            self.api_operation.replace(cmd.api_operation().clone());
            let response = self.responses.borrow_mut().pop().unwrap();
            self.http_method.replace(cmd.method.clone());
            match response.status {
                // 409 Conflict - Merge request already exists. - Gitlab
                // 422 Conflict - Merge request already exists. - Github
                200 | 201 | 302 | 409 | 422 => return Ok(response),
                // RateLimit error code. 403 secondary rate limit, 429 primary
                // rate limit.
                403 | 429 => {
                    let headers = response.get_ratelimit_headers().unwrap_or_default();
                    return Err(error::GRError::RateLimitExceeded(headers).into());
                }
                _ => return Err(error::gen(&response.body)),
            }
        }

        fn api_max_pages<T: Serialize>(&self, _cmd: &Request<T>) -> u32 {
            self.config.get_max_pages(
                &self
                    .api_operation
                    .borrow()
                    .as_ref()
                    // We set it to Project by default in cases where it does
                    // not matter while testing.
                    .unwrap_or(&ApiOperation::Project),
            )
        }

        fn throttle(&self, milliseconds: Milliseconds) {
            let mut throttled = self.throttled.borrow_mut();
            *throttled += 1;
            let mut milliseconds_throttled = self.milliseconds_throttled.borrow_mut();
            *milliseconds_throttled += milliseconds;
        }
    }

    pub struct ConfigMock {
        max_pages: u32,
    }

    impl ConfigMock {
        pub fn new(max_pages: u32) -> Self {
            ConfigMock { max_pages }
        }
    }

    impl ConfigProperties for ConfigMock {
        fn api_token(&self) -> &str {
            "1234"
        }
        fn cache_location(&self) -> &str {
            ""
        }
        fn get_max_pages(&self, _api_operation: &ApiOperation) -> u32 {
            self.max_pages
        }
    }

    pub fn config() -> impl ConfigProperties {
        ConfigMock::default()
    }

    impl Default for ConfigMock {
        fn default() -> Self {
            ConfigMock {
                max_pages: REST_API_MAX_PAGES,
            }
        }
    }

    impl ConfigProperties for Arc<ConfigMock> {
        fn api_token(&self) -> &str {
            "1234"
        }
        fn cache_location(&self) -> &str {
            ""
        }
        fn get_max_pages(&self, _api_operation: &ApiOperation) -> u32 {
            self.as_ref().max_pages
        }
    }

    struct TestLogger;

    lazy_static! {
        pub static ref LOG_BUFFER: Mutex<String> = Mutex::new(String::new());
    }

    impl log::Log for TestLogger {
        fn enabled(&self, metadata: &Metadata) -> bool {
            metadata.level() <= Level::Trace
        }

        fn log(&self, record: &Record) {
            if self.enabled(record.metadata()) {
                let mut buffer = LOG_BUFFER.lock().unwrap();
                writeln!(buffer, "{} - {}", record.level(), record.args())
                    .expect("Failed to write to log buffer");
            }
        }

        fn flush(&self) {}
    }

    pub fn init_test_logger() {
        let logger = TestLogger;
        log::set_boxed_logger(Box::new(logger)).expect("Failed to set logger");
        log::set_max_level(LevelFilter::Trace);
    }
}
