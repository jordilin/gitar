//! Config file parsing and validation.

use crate::api_defaults::{RATE_LIMIT_REMAINING_THRESHOLD, REST_API_MAX_PAGES};
use crate::api_traits::ApiOperation;
use crate::error;
use crate::Result;
use std::sync::Arc;
use std::{collections::HashMap, io::Read};

pub trait ConfigProperties {
    fn api_token(&self) -> &str;
    fn cache_location(&self) -> &str;
    fn preferred_assignee_username(&self) -> &str {
        ""
    }
    fn merge_request_description_signature(&self) -> &str {
        ""
    }
    fn get_cache_expiration(&self, _api_operation: &ApiOperation) -> &str {
        // Defaults to regular HTTP cache expiration mechanisms.
        "0s"
    }
    fn get_max_pages(&self, _api_operation: &ApiOperation) -> u32 {
        REST_API_MAX_PAGES
    }

    fn rate_limit_remaining_threshold(&self) -> u32 {
        RATE_LIMIT_REMAINING_THRESHOLD
    }
}

#[derive(Clone, Default)]
pub struct Config {
    api_token: String,
    cache_location: String,
    preferred_assignee_username: String,
    merge_request_description_signature: String,
    // TODO, should be <ApiOperation, Seconds>
    cache_expirations: HashMap<ApiOperation, String>,
    max_pages: HashMap<ApiOperation, u32>,
    rate_limit_remaining_threshold: u32,
}

impl Config {
    // TODO: make use of a BufReader instead
    pub fn new<T: Read>(reader: T, domain: &str) -> Result<Self> {
        let config = Config::parse(reader, domain)?;
        let domain_config_data = config.get(domain).unwrap();
        // ENV VAR API token takes preference. For a given domain, we try to fetch
        // <DOMAIN>_API_TOKEN env var first, then we fallback to the config
        // file. Given a domain such as gitlab.com, the env var to be set is
        // GITLAB_API_TOKEN. If the domain is gitlab.<company>.com, the env var
        // to be set is GITLAB_<COMPANY>_API_TOKEN.

        // Remove top level domain, if any (e.g. gitlab.com -> gitlab)
        let domain_fields = domain.split('.').collect::<Vec<&str>>();
        let env_domain = if domain_fields.len() == 1 {
            // There's not top level domain, such as .com
            domain
        } else {
            &domain_fields[0..domain_fields.len() - 1].join("_")
        };
        let api_token = std::env::var(format!("{}_API_TOKEN", env_domain.to_ascii_uppercase()))
            .or_else(|_| -> Result<String> {
                let token_res = domain_config_data.get("api_token").ok_or_else(|| {
                    error::gen(format!(
                        "No api_token found for domain {} in config",
                        domain
                    ))
                })?;
                Ok(token_res.to_string())
            })?;
        let cache_location = domain_config_data.get("cache_location").ok_or_else(|| {
            error::gen(format!(
                "No cache_location found for domain {} in config",
                domain
            ))
        })?;
        let default_assignee_username = "".to_string();
        let preferred_assignee_username = domain_config_data
            .get("preferred_assignee_username")
            .unwrap_or(&default_assignee_username);
        let default_merge_request_description_signature = "".to_string();
        let merge_request_description_signature = domain_config_data
            .get("merge_request_description_signature")
            .unwrap_or(&default_merge_request_description_signature);
        let cache_expirations = Config::cache_expirations(domain_config_data);
        let max_pages = Config::max_pages(domain_config_data);
        let rate_limit_remaining_threshold = domain_config_data
            .get("rate_limit_remaining_threshold")
            .and_then(|s| s.parse().ok())
            .unwrap_or(RATE_LIMIT_REMAINING_THRESHOLD);

        Ok(Config {
            api_token: api_token.to_string(),
            cache_location: cache_location.to_string(),
            preferred_assignee_username: preferred_assignee_username.to_string(),
            merge_request_description_signature: merge_request_description_signature.to_string(),
            cache_expirations,
            max_pages,
            rate_limit_remaining_threshold,
        })
    }

    fn max_pages(domain_config_data: &HashMap<String, String>) -> HashMap<ApiOperation, u32> {
        let mut max_pages: HashMap<ApiOperation, u32> = HashMap::new();
        max_pages.insert(
            ApiOperation::MergeRequest,
            domain_config_data
                .get("max_pages_api_merge_request")
                .and_then(|s| s.parse().ok())
                .unwrap_or(REST_API_MAX_PAGES),
        );
        max_pages.insert(
            ApiOperation::Pipeline,
            domain_config_data
                .get("max_pages_api_pipeline")
                .and_then(|s| s.parse().ok())
                .unwrap_or(REST_API_MAX_PAGES),
        );
        max_pages.insert(
            ApiOperation::Project,
            domain_config_data
                .get("max_pages_api_project")
                .and_then(|s| s.parse().ok())
                .unwrap_or(REST_API_MAX_PAGES),
        );
        max_pages.insert(
            ApiOperation::ContainerRegistry,
            domain_config_data
                .get("max_pages_api_container_registry")
                .and_then(|s| s.parse().ok())
                .unwrap_or(REST_API_MAX_PAGES),
        );
        max_pages.insert(
            ApiOperation::Release,
            domain_config_data
                .get("max_pages_api_release")
                .and_then(|s| s.parse().ok())
                .unwrap_or(REST_API_MAX_PAGES),
        );
        max_pages.insert(
            ApiOperation::Gist,
            domain_config_data
                .get("max_pages_api_gist")
                .and_then(|s| s.parse().ok())
                .unwrap_or(REST_API_MAX_PAGES),
        );
        max_pages
    }

    fn cache_expirations(
        domain_config_data: &HashMap<String, String>,
    ) -> HashMap<ApiOperation, String> {
        let mut cache_expirations: HashMap<ApiOperation, String> = HashMap::new();
        cache_expirations.insert(
            ApiOperation::MergeRequest,
            domain_config_data
                .get("cache_api_merge_request_expiration")
                .unwrap_or(&"0s".to_string())
                .to_string(),
        );
        cache_expirations.insert(
            ApiOperation::Pipeline,
            domain_config_data
                .get("cache_api_pipeline_expiration")
                .unwrap_or(&"0s".to_string())
                .to_string(),
        );
        cache_expirations.insert(
            ApiOperation::Project,
            domain_config_data
                .get("cache_api_project_expiration")
                .unwrap_or(&"0s".to_string())
                .to_string(),
        );
        cache_expirations.insert(
            ApiOperation::ContainerRegistry,
            domain_config_data
                .get("cache_api_container_registry_expiration")
                .unwrap_or(&"0s".to_string())
                .to_string(),
        );
        cache_expirations.insert(
            ApiOperation::Release,
            domain_config_data
                .get("cache_api_release_expiration")
                .unwrap_or(&"0s".to_string())
                .to_string(),
        );
        cache_expirations.insert(
            ApiOperation::SinglePage,
            domain_config_data
                .get("cache_api_single_page_expiration")
                .unwrap_or(&"0s".to_string())
                .to_string(),
        );
        cache_expirations.insert(
            ApiOperation::Gist,
            domain_config_data
                .get("cache_api_gist_expiration")
                .unwrap_or(&"0s".to_string())
                .to_string(),
        );
        cache_expirations
    }

    fn parse<T: Read>(
        mut reader: T,
        domain: &str,
    ) -> Result<HashMap<String, HashMap<String, String>>> {
        let mut config_data = String::new();
        reader.read_to_string(&mut config_data)?;
        let lines = config_data.lines();
        let mut config = HashMap::new();
        let mut domain_config = HashMap::new();

        let regex =
            regex::Regex::new(&format!(r"^{}\.(?P<key>\w+)=(?P<value>.*)", domain)).unwrap();
        for line in lines {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            // capture groups key and value from regex
            let captured_names = regex.captures(line);
            match captured_names {
                Some(captured_names) => {
                    let key = captured_names.name("key").unwrap().as_str();
                    let value = captured_names.name("value").unwrap().as_str();
                    domain_config.insert(key.to_string(), value.to_string());
                }
                None => {
                    continue;
                }
            }
        }

        config.insert(domain.to_string(), domain_config);
        if config.is_empty() {
            return Err(error::gen("No config data found"));
        }
        Ok(config)
    }
}

impl ConfigProperties for Config {
    fn api_token(&self) -> &str {
        &self.api_token
    }

    fn cache_location(&self) -> &str {
        &self.cache_location
    }

    fn preferred_assignee_username(&self) -> &str {
        &self.preferred_assignee_username
    }

    fn merge_request_description_signature(&self) -> &str {
        &self.merge_request_description_signature
    }

    fn get_cache_expiration(&self, api_operation: &ApiOperation) -> &str {
        let expiration = self.cache_expirations.get(api_operation);
        match expiration {
            Some(expiration) => expiration,
            None => "",
        }
    }

    fn get_max_pages(&self, api_operation: &ApiOperation) -> u32 {
        let max_pages = self.max_pages.get(api_operation);
        match max_pages {
            Some(max_pages) => *max_pages,
            None => REST_API_MAX_PAGES,
        }
    }

    fn rate_limit_remaining_threshold(&self) -> u32 {
        self.rate_limit_remaining_threshold
    }
}

impl ConfigProperties for Arc<Config> {
    fn api_token(&self) -> &str {
        self.as_ref().api_token()
    }

    fn cache_location(&self) -> &str {
        self.as_ref().cache_location()
    }

    fn preferred_assignee_username(&self) -> &str {
        self.as_ref().preferred_assignee_username()
    }

    fn merge_request_description_signature(&self) -> &str {
        self.as_ref().merge_request_description_signature()
    }

    fn get_cache_expiration(&self, api_operation: &ApiOperation) -> &str {
        self.as_ref().get_cache_expiration(api_operation)
    }

    fn get_max_pages(&self, api_operation: &ApiOperation) -> u32 {
        self.as_ref().get_max_pages(api_operation)
    }

    fn rate_limit_remaining_threshold(&self) -> u32 {
        self.as_ref().rate_limit_remaining_threshold()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_get_api_token() {
        let config_data = r#"
        gitlab.com.api_token=1234
        github.com.api_token=4567
        gitlab.com.cache_location=/home/user/.config/mr_cache
        github.com.cache_location=/home/user/.config/mr_cache
        "#;
        let domain = "gitlab.com";
        let reader = std::io::Cursor::new(config_data);
        let config = Arc::new(Config::new(reader, domain).unwrap());
        assert_eq!("1234", config.api_token());
    }

    #[test]
    fn test_ignore_commented_out_lines_and_empty_lines() {
        let config_data = r#"

        # api token
        gitlab.com.api_token=1234
        github.com.api_token=4567
        gitlab.com.cache_location=/home/user/.config/mr_cache
        github.com.cache_location=/home/user/.config/mr_cache
        "#;
        let domain = "gitlab.com";
        let reader = std::io::Cursor::new(config_data);
        let config = Arc::new(Config::new(reader, domain).unwrap());
        assert_eq!("1234", config.api_token());
    }

    #[test]
    fn test_no_api_token_is_err() {
        let config_data = r#"
        # api token
        gitlab.com.api_token_typo=1234"#;
        let domain = "gitlab.com";
        let reader = std::io::Cursor::new(config_data);
        assert!(Config::new(reader, domain).is_err());
    }

    #[test]
    fn test_config_no_data() {
        let config_data = "";
        let domain = "gitlab.com";
        let reader = std::io::Cursor::new(config_data);
        assert!(Config::new(reader, domain).is_err());
    }

    #[test]
    fn test_config_multiple_equals() {
        let config_data = "gitlab_api_token===1234";
        let domain = "gitlab.com";
        let reader = std::io::Cursor::new(config_data);
        assert!(Config::new(reader, domain).is_err());
    }

    #[test]
    fn test_get_preferred_assignee_username() {
        let config_data = r#"
        github.com.api_token=1234
        github.com.cache_location=/home/user/.config/mr_cache
        github.com.preferred_assignee_username=jordilin"#;
        let domain = "github.com";
        let reader = std::io::Cursor::new(config_data);
        let config = Arc::new(Config::new(reader, domain).unwrap());
        assert_eq!("jordilin", config.preferred_assignee_username());
    }

    #[test]
    fn test_get_merge_request_description_signature() {
        let config_data = r#"
        github.com.api_token=1234
        github.com.cache_location=/home/user/.config/mr_cache
        github.com.preferred_assignee_username=jordilin
        github.com.merge_request_description_signature=- devops team :-)"#;
        let domain = "github.com";
        let reader = std::io::Cursor::new(config_data);
        let config = Arc::new(Config::new(reader, domain).unwrap());
        assert_eq!(
            "- devops team :-)",
            config.merge_request_description_signature()
        );
    }

    #[test]
    fn test_config_cache_api_expirations() {
        let config_data = r#"
        github.com.api_token=1234
        github.com.cache_location=/home/user/.config/mr_cache
        github.com.preferred_assignee_username=jordilin
        github.com.merge_request_description_signature=- devops team :-)
        github.com.cache_api_merge_request_expiration=2h
        github.com.cache_api_pipeline_expiration=1h
        github.com.cache_api_project_expiration=3h
        github.com.cache_api_container_registry_expiration=4h
        github.com.cache_api_single_page_expiration=1d
        github.com.cache_api_release_expiration=5h
        github.com.cache_api_gist_expiration=1d"#;
        let domain = "github.com";
        let reader = std::io::Cursor::new(config_data);
        let config = Arc::new(Config::new(reader, domain).unwrap());
        assert_eq!(
            "2h",
            config.get_cache_expiration(&ApiOperation::MergeRequest)
        );
        assert_eq!("1h", config.get_cache_expiration(&ApiOperation::Pipeline));
        assert_eq!("3h", config.get_cache_expiration(&ApiOperation::Project));
        assert_eq!(
            "4h",
            config.get_cache_expiration(&ApiOperation::ContainerRegistry)
        );
        assert_eq!("5h", config.get_cache_expiration(&ApiOperation::Release));
        assert_eq!("1d", config.get_cache_expiration(&ApiOperation::SinglePage));
        assert_eq!("1d", config.get_cache_expiration(&ApiOperation::Gist));
    }

    #[test]
    fn test_config_cache_api_expiration_default() {
        let config_data = r#"
        github.com.api_token=1234
        github.com.cache_location=/home/user/.config/mr_cache
        github.com.preferred_assignee_username=jordilin
        github.com.merge_request_description_signature=- devops team :-)
        "#;
        let domain = "github.com";
        let reader = std::io::Cursor::new(config_data);
        let config = Arc::new(Config::new(reader, domain).unwrap());
        assert_eq!(
            "0s",
            config.get_cache_expiration(&ApiOperation::MergeRequest)
        );
    }

    #[test]
    fn test_config_max_pages_merge_requests() {
        let config_data = r#"
        github.com.api_token=1234
        github.com.cache_location=/home/user/.config/mr_cache
        github.com.preferred_assignee_username=jordilin
        github.com.max_pages_api_merge_request=2
        "#;
        let domain = "github.com";
        let reader = std::io::Cursor::new(config_data);
        let config = Arc::new(Config::new(reader, domain).unwrap());
        assert_eq!(2, config.get_max_pages(&ApiOperation::MergeRequest));
    }

    #[test]
    fn test_config_max_pages_default_merge_request() {
        let config_data = r#"
        github.com.api_token=1234
        github.com.cache_location=/home/user/.config/mr_cache
        github.com.preferred_assignee_username=jordilin
        "#;
        let domain = "github.com";
        let reader = std::io::Cursor::new(config_data);
        let config = Arc::new(Config::new(reader, domain).unwrap());
        assert_eq!(
            REST_API_MAX_PAGES,
            config.get_max_pages(&ApiOperation::MergeRequest)
        );
    }

    #[test]
    fn test_config_max_pages_pipeline() {
        let config_data = r#"
        github.com.api_token=1234
        github.com.cache_location=/home/user/.config/mr_cache
        github.com.preferred_assignee_username=jordilin
        github.com.max_pages_api_pipeline=4
        "#;
        let domain = "github.com";
        let reader = std::io::Cursor::new(config_data);
        let config = Arc::new(Config::new(reader, domain).unwrap());
        assert_eq!(4, config.get_max_pages(&ApiOperation::Pipeline));
    }

    #[test]
    fn test_config_max_pages_default_pipeline() {
        let config_data = r#"
        github.com.api_token=1234
        github.com.cache_location=/home/user/.config/mr_cache
        github.com.preferred_assignee_username=jordilin"#;
        let domain = "github.com";
        let reader = std::io::Cursor::new(config_data);
        let config = Arc::new(Config::new(reader, domain).unwrap());
        assert_eq!(
            REST_API_MAX_PAGES,
            config.get_max_pages(&ApiOperation::Pipeline)
        );
    }

    #[test]
    fn test_config_max_pages_project() {
        let config_data = r#"
        github.com.api_token=1234
        github.com.cache_location=/home/user/.config/mr_cache
        github.com.preferred_assignee_username=jordilin
        github.com.max_pages_api_project=6
        "#;
        let domain = "github.com";
        let reader = std::io::Cursor::new(config_data);
        let config = Arc::new(Config::new(reader, domain).unwrap());
        assert_eq!(6, config.get_max_pages(&ApiOperation::Project));
    }

    #[test]
    fn test_config_max_pages_default_project() {
        let config_data = r#"
        github.com.api_token=1234
        github.com.cache_location=/home/user/.config/mr_cache
        github.com.preferred_assignee_username=jordilin"#;
        let domain = "github.com";
        let reader = std::io::Cursor::new(config_data);
        let config = Arc::new(Config::new(reader, domain).unwrap());
        assert_eq!(
            REST_API_MAX_PAGES,
            config.get_max_pages(&ApiOperation::Project)
        );
    }

    #[test]
    fn test_get_rate_limit_remaining_threshold() {
        let config_data = r#"
        gitlab.com.api_token=1234
        gitlab.com.cache_location=/home/user/.config/mr_cache
        gitlab.com.rate_limit_remaining_threshold=15
        "#;
        let domain = "gitlab.com";
        let reader = std::io::Cursor::new(config_data);
        let config = Arc::new(Config::new(reader, domain).unwrap());
        assert_eq!(15, config.rate_limit_remaining_threshold());
    }

    #[test]
    fn test_get_max_pages_for_container_registry_operations() {
        let config_data = r#"
        gitlab.com.api_token=1234
        gitlab.com.cache_location=/home/user/.config/mr_cache
        gitlab.com.max_pages_api_container_registry=15
        "#;
        let domain = "gitlab.com";
        let reader = std::io::Cursor::new(config_data);
        let config = Arc::new(Config::new(reader, domain).unwrap());
        assert_eq!(15, config.get_max_pages(&ApiOperation::ContainerRegistry));
    }

    #[test]
    fn test_get_max_pages_for_read_releases() {
        let config_data = r#"
        gitlab.com.api_token=1234
        gitlab.com.cache_location=/home/user/.config/mr_cache
        gitlab.com.max_pages_api_release=15
        "#;
        let domain = "gitlab.com";
        let reader = std::io::Cursor::new(config_data);
        let config = Arc::new(Config::new(reader, domain).unwrap());
        assert_eq!(15, config.get_max_pages(&ApiOperation::Release));
    }

    #[test]
    fn test_get_max_pages_for_gists() {
        let config_data = r#"
        gitlab.com.api_token=1234
        gitlab.com.cache_location=/home/user/.config/mr_cache
        gitlab.com.max_pages_api_gist=15
        "#;
        let domain = "gitlab.com";
        let reader = std::io::Cursor::new(config_data);
        let config = Arc::new(Config::new(reader, domain).unwrap());
        assert_eq!(15, config.get_max_pages(&ApiOperation::Gist));
    }

    #[test]
    fn test_use_gitlab_com_api_token_envvar() {
        let config_data = r#"
        gitlab.com.cache_location=/home/user/.config/mr_cache
        gitlab.com.rate_limit_remaining_threshold=15
        "#;
        let domain = "gitlab.com";
        let reader = std::io::Cursor::new(config_data);
        std::env::set_var("GITLAB_API_TOKEN", "1234");
        let config = Arc::new(Config::new(reader, domain).unwrap());
        assert_eq!("1234", config.api_token());
    }

    #[test]
    fn test_use_github_com_api_token_envvar() {
        let config_data = r#"
        github.com.cache_location=/home/user/.config/mr_cache
        github.com.rate_limit_remaining_threshold=15
        "#;
        let domain = "github.com";
        let reader = std::io::Cursor::new(config_data);
        std::env::set_var("GITHUB_API_TOKEN", "4567");
        let config = Arc::new(Config::new(reader, domain).unwrap());
        assert_eq!("4567", config.api_token());
    }

    #[test]
    fn test_use_sub_domain_gitlab_token_env_var() {
        let config_data = r#"
        gitlab.company.com.cache_location=/home/user/.config/mr_cache
        gitlab.company.com.rate_limit_remaining_threshold=15
        "#;
        let domain = "gitlab.company.com";
        let reader = std::io::Cursor::new(config_data);
        std::env::set_var("GITLAB_COMPANY_API_TOKEN", "1214");
        let config = Arc::new(Config::new(reader, domain).unwrap());
        assert_eq!("1214", config.api_token());
    }

    #[test]
    fn test_domain_without_top_leven_domain_token_envvar() {
        let config_data = r#"
        gitlabweb.cache_location=/home/user/.config/mr_cache
        gitlabweb.rate_limit_remaining_threshold=15
        "#;
        let domain = "gitlabweb";
        let reader = std::io::Cursor::new(config_data);
        std::env::set_var("GITLABWEB_API_TOKEN", "1294");
        let config = Arc::new(Config::new(reader, domain).unwrap());
        assert_eq!("1294", config.api_token());
    }
}
