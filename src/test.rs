#[cfg(test)]
pub mod utils {
    use crate::http::Headers;
    use crate::{api_traits::ApiOperation, config::ConfigProperties, error};
    use serde::Serialize;

    use crate::api_defaults::REST_API_MAX_PAGES;

    use crate::{
        http::Request,
        io::{HttpRunner, Response, TaskRunner},
        Result,
    };
    use std::sync::Arc;
    use std::{
        cell::{Ref, RefCell},
        fs::File,
        io::Read,
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

    use std::collections::HashMap;
    use std::sync::Mutex;

    lazy_static::lazy_static! {
        static ref CACHE: Mutex<HashMap<String, String>> = Mutex::new(HashMap::new());
    }

    pub fn get_contract(contract_type: ContractType, filename: &str) -> String {
        let contracts_path = format!("contracts/{}/{}", contract_type.as_str(), filename);
        let mut cache = CACHE.lock().unwrap();
        if let Some(contents) = cache.get(&contracts_path) {
            return contents.clone();
        }
        let mut file = File::open(&contracts_path).unwrap();
        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();
        cache.insert(contracts_path, contents.clone());
        contents
    }

    pub struct MockRunner {
        responses: RefCell<Vec<Response>>,
        cmd: RefCell<String>,
        headers: RefCell<Headers>,
        url: RefCell<String>,
        pub api_operation: RefCell<Option<ApiOperation>>,
        pub config: ConfigMock,
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
            match response.status {
                // 409 Conflict - Merge request already exists. - Gitlab
                // 422 Conflict - Merge request already exists. - Github
                200 | 201 | 302 | 409 | 422 => return Ok(response),
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
}
