#[cfg(test)]
pub mod utils {
    use crate::{config::ConfigProperties, error};
    use serde::Serialize;

    use crate::{
        http::Request,
        io::{HttpRunner, Response, Runner},
        Result,
    };
    use std::{
        cell::{Ref, RefCell},
        collections::HashMap,
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
        headers: RefCell<HashMap<String, String>>,
        url: RefCell<String>,
    }

    impl MockRunner {
        pub fn new(responses: Vec<Response>) -> Self {
            Self {
                responses: RefCell::new(responses),
                cmd: RefCell::new(String::new()),
                headers: RefCell::new(HashMap::new()),
                url: RefCell::new(String::new()),
            }
        }

        pub fn cmd(&self) -> Ref<String> {
            self.cmd.borrow()
        }

        pub fn url(&self) -> Ref<String> {
            self.url.borrow()
        }

        pub fn headers(&self) -> Ref<HashMap<String, String>> {
            self.headers.borrow()
        }
    }

    impl Runner for MockRunner {
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
            match response.status() {
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
            let response = self.responses.borrow_mut().pop().unwrap();
            match response.status() {
                // 409 Conflict - Merge request already exists.
                200 | 201 | 409 => return Ok(response),
                _ => return Err(error::gen(&response.body)),
            }
        }
    }

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
            ""
        }
    }

    pub fn config() -> impl ConfigProperties {
        ConfigMock::new()
    }
}
