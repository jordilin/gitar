#[cfg(test)]
pub mod utils {
    use crate::{
        api_defaults::REST_API_MAX_PAGES,
        api_traits::ApiOperation,
        config::ConfigProperties,
        error,
        http::{
            self,
            throttle::{self, ThrottleStrategyType},
            Headers, Request,
        },
        io::{self, HttpResponse, HttpRunner, ShellResponse, TaskRunner},
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
        ops::Deref,
        rc::Rc,
        sync::{Arc, Mutex},
    };

    #[derive(Debug, Clone, Copy, PartialEq)]
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

    pub struct MockRunner<R> {
        responses: RefCell<Vec<R>>,
        cmd: RefCell<String>,
        headers: RefCell<Headers>,
        url: RefCell<String>,
        pub api_operation: RefCell<Option<ApiOperation>>,
        pub config: ConfigMock,
        pub http_method: RefCell<Vec<http::Method>>,
        pub throttled: RefCell<u32>,
        pub milliseconds_throttled: RefCell<Milliseconds>,
        pub run_count: RefCell<u32>,
        pub request_body: RefCell<String>,
    }

    impl<R> MockRunner<R> {
        pub fn new(responses: Vec<R>) -> Self {
            Self {
                responses: RefCell::new(responses),
                cmd: RefCell::new(String::new()),
                headers: RefCell::new(Headers::new()),
                url: RefCell::new(String::new()),
                api_operation: RefCell::new(None),
                config: ConfigMock::default(),
                http_method: RefCell::new(Vec::new()),
                throttled: RefCell::new(0),
                milliseconds_throttled: RefCell::new(Milliseconds::new(0)),
                run_count: RefCell::new(0),
                request_body: RefCell::new(String::new()),
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

        pub fn request_body(&self) -> Ref<String> {
            self.request_body.borrow()
        }
    }

    impl TaskRunner for MockRunner<ShellResponse> {
        type Response = ShellResponse;

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
            *self.run_count.borrow_mut() += 1;
            match response.status {
                0 => Ok(response),
                _ => Err(error::gen(&response.body)),
            }
        }
    }

    impl HttpRunner for MockRunner<HttpResponse> {
        type Response = HttpResponse;

        fn run<T: Serialize>(&self, cmd: &mut Request<T>) -> Result<Self::Response> {
            self.url.replace(cmd.url().to_string());
            self.headers.replace(cmd.headers().clone());
            self.api_operation.replace(cmd.api_operation().clone());
            let response = self.responses.borrow_mut().pop().unwrap();
            let body = serde_json::to_string(&cmd.body).unwrap_or_default();
            self.request_body.replace(body);
            self.http_method.borrow_mut().push(cmd.method.clone());
            match response.status {
                // 409 Conflict - Merge request already exists. - Gitlab
                // 422 Conflict - Merge request already exists. - Github
                200 | 201 | 302 | 409 | 422 => Ok(response),
                // RateLimit error code. 403 secondary rate limit, 429 primary
                // rate limit.
                403 | 429 => {
                    let headers = response.get_ratelimit_headers().unwrap_or_default();
                    Err(error::GRError::RateLimitExceeded(headers).into())
                }
                500..=599 => Err(error::GRError::RemoteServerError(response.body).into()),
                // Just for testing purposes, if the test client sets a status
                // code of -1 we return a HTTP transport error.
                -1 => Err(error::GRError::HttpTransportError(response.body).into()),
                _ => Err(error::gen(&response.body)),
            }
        }

        fn api_max_pages<T: Serialize>(&self, _cmd: &Request<T>) -> u32 {
            self.config.get_max_pages(
                self.api_operation
                    .borrow()
                    .as_ref()
                    // We set it to Project by default in cases where it does
                    // not matter while testing.
                    .unwrap_or(&ApiOperation::Project),
            )
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
        fn cache_location(&self) -> Option<&str> {
            Some("")
        }
        fn get_max_pages(&self, _api_operation: &ApiOperation) -> u32 {
            self.max_pages
        }
    }

    pub fn config() -> Arc<dyn ConfigProperties> {
        Arc::new(ConfigMock::default())
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
        fn cache_location(&self) -> Option<&str> {
            Some("")
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
        log::set_boxed_logger(Box::new(logger)).unwrap_or(());
        log::set_max_level(LevelFilter::Trace);
    }

    pub struct Domain(pub String);
    pub struct BasePath(pub String);

    impl Deref for Domain {
        type Target = String;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl Deref for BasePath {
        type Target = String;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    pub enum ClientType {
        Gitlab(Domain, BasePath),
        Github(Domain, BasePath),
    }

    pub fn default_gitlab() -> ClientType {
        ClientType::Gitlab(
            Domain("gitlab.com".to_string()),
            BasePath("jordilin/gitlapi".to_string()),
        )
    }

    pub fn default_github() -> ClientType {
        ClientType::Github(
            Domain("github.com".to_string()),
            BasePath("jordilin/githapi".to_string()),
        )
    }

    #[macro_export]
    macro_rules! setup_client {
        ($response_contracts:expr, $client_type:expr, $trait_type:ty) => {{
            let config = $crate::test::utils::config();
            let responses: Vec<_> = $response_contracts
                .into_iter()
                .map(|(status_code, get_contract_fn, headers)| {
                    let body = get_contract_fn();
                    let mut response = HttpResponse::builder();
                    response.status(status_code);
                    if headers.is_some() {
                        response.headers(headers.clone().unwrap());
                        let rate_limit_header =
                            crate::io::parse_ratelimit_headers(headers.as_ref());
                        let link_header = crate::io::parse_page_headers(headers.as_ref());
                        let flow_control_headers = crate::io::FlowControlHeaders::new(
                            std::rc::Rc::new(link_header),
                            std::rc::Rc::new(rate_limit_header),
                        );
                        response.flow_control_headers(flow_control_headers);
                    }
                    if body.is_some() {
                        response.body(body.unwrap());
                    }
                    response.build().unwrap()
                })
                .collect();
            let client = std::sync::Arc::new(crate::test::utils::MockRunner::new(responses));
            let remote: Box<$trait_type> = match $client_type {
                crate::test::utils::ClientType::Gitlab(domain, path) => Box::new(
                    crate::gitlab::Gitlab::new(config, &domain, &path, client.clone()),
                ),
                crate::test::utils::ClientType::Github(domain, path) => Box::new(
                    crate::github::Github::new(config, &domain, &path, client.clone()),
                ),
            };

            (client, remote)
        }};
    }

    pub struct ResponseContracts {
        contract_type: ContractType,
        contracts: Vec<(i32, Box<dyn Fn() -> Option<String>>, Option<Headers>)>,
    }

    impl ResponseContracts {
        pub fn new(contract_type: ContractType) -> Self {
            Self {
                contract_type,
                contracts: Vec::new(),
            }
        }

        pub fn add_body<B: Into<String> + Clone + 'static>(
            mut self,
            status_code: i32,
            body: Option<B>,
            headers: Option<Headers>,
        ) -> Self {
            self.contracts.push((
                status_code,
                Box::new(move || body.clone().map(|b| b.into())),
                headers,
            ));
            self
        }

        pub fn add_contract<F: Into<String> + Clone + 'static>(
            mut self,
            status_code: i32,
            contract_file: F,
            headers: Option<Headers>,
        ) -> Self {
            self.contracts.push((
                status_code,
                Box::new(move || {
                    Some(get_contract(
                        self.contract_type,
                        &contract_file.clone().into(),
                    ))
                }),
                headers,
            ));
            self
        }
    }

    impl IntoIterator for ResponseContracts {
        type Item = (i32, Box<dyn Fn() -> Option<String>>, Option<Headers>);
        type IntoIter = std::vec::IntoIter<Self::Item>;

        fn into_iter(self) -> Self::IntoIter {
            self.contracts.into_iter()
        }
    }

    pub struct MockThrottler {
        throttled: RefCell<u32>,
        milliseconds_throttled: RefCell<Milliseconds>,
        strategy: throttle::ThrottleStrategyType,
    }

    impl MockThrottler {
        pub fn new(strategy_type: Option<ThrottleStrategyType>) -> Self {
            Self {
                throttled: RefCell::new(0),
                milliseconds_throttled: RefCell::new(Milliseconds::new(0)),
                strategy: strategy_type.unwrap_or(ThrottleStrategyType::NoThrottle),
            }
        }

        pub fn throttled(&self) -> Ref<u32> {
            self.throttled.borrow()
        }

        pub fn milliseconds_throttled(&self) -> Ref<Milliseconds> {
            self.milliseconds_throttled.borrow()
        }
    }

    impl http::throttle::ThrottleStrategy for Rc<MockThrottler> {
        fn throttle(&self, _response: Option<&io::FlowControlHeaders>) {
            let mut throttled = self.throttled.borrow_mut();
            *throttled += 1;
        }

        fn throttle_for(&self, delay: Milliseconds) {
            let mut throttled = self.throttled.borrow_mut();
            *throttled += 1;
            let mut milliseconds_throttled = self.milliseconds_throttled.borrow_mut();
            *milliseconds_throttled += delay;
        }

        fn strategy(&self) -> ThrottleStrategyType {
            self.strategy.clone()
        }
    }
}
