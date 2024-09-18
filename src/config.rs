//! Config file parsing and validation.

use crate::api_defaults::{EXPIRE_IMMEDIATELY, RATE_LIMIT_REMAINING_THRESHOLD, REST_API_MAX_PAGES};
use crate::api_traits::ApiOperation;
use crate::cmds::project::Member;
use crate::error::{self, GRError};
use crate::Result;
use serde::Deserialize;
use std::sync::Arc;
use std::{collections::HashMap, io::Read};

pub trait ConfigProperties: Send + Sync {
    fn api_token(&self) -> &str;
    fn cache_location(&self) -> Option<&str>;
    fn preferred_assignee_username(&self) -> Option<Member> {
        None
    }

    fn merge_request_members(&self) -> Option<Vec<Member>> {
        None
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

/// The NoConfig struct is used when no configuration is found and it can be
/// used for CI/CD scenarios where no configuration is needed or for other
/// one-off scenarios.
pub struct NoConfig {
    api_token: String,
}

impl NoConfig {
    pub fn new<FE: Fn(&str) -> Result<String>>(domain: &str, env: FE) -> Result<Self> {
        let api_token_res = env(domain);
        let api_token = api_token_res.map_err(|_| {
            GRError::PreconditionNotMet(format!(
                "Configuration not found, so it is expected environment variable {}_API_TOKEN to be set.",
                env_var(domain)
            ))
        })?;
        Ok(NoConfig { api_token })
    }
}

impl ConfigProperties for NoConfig {
    fn api_token(&self) -> &str {
        &self.api_token
    }

    fn cache_location(&self) -> Option<&str> {
        None
    }
}

#[derive(Deserialize, Clone, Debug)]
struct ApiSettings {
    #[serde(flatten)]
    settings: HashMap<ApiOperation, String>,
}

#[derive(Deserialize, Clone, Debug)]
struct MaxPagesApi {
    #[serde(flatten)]
    settings: HashMap<ApiOperation, u32>,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
enum UserInfo {
    /// Github remote. Github REST API requires username only when using the
    /// REST API.
    UsernameOnly(String),
    /// Gitlab remotes. Gitlab REST API requires user ID. This configuration
    /// allows us to map username with user ID, so we can identify which ID is
    /// associated to which user.
    UsernameID { username: String, id: u64 },
}

#[derive(Deserialize, Clone, Debug, Default)]
struct MergeRequestConfig {
    preferred_assignee_username: Option<UserInfo>,
    members: Option<Vec<UserInfo>>,
    description_signature: Option<String>,
}

#[derive(Deserialize, Clone, Debug)]
struct ProjectConfig {
    merge_requests: Option<MergeRequestConfig>,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct DomainConfig {
    api_token: Option<String>,
    cache_location: Option<String>,
    merge_requests: Option<MergeRequestConfig>,
    rate_limit_remaining_threshold: Option<u32>,
    cache_expirations: Option<ApiSettings>,
    max_pages_api: Option<MaxPagesApi>,
    #[serde(flatten)]
    projects: HashMap<String, ProjectConfig>,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct ConfigFileInner {
    #[serde(flatten)]
    domains: HashMap<String, DomainConfig>,
}

#[derive(Clone, Debug, Default)]
pub struct ConfigFile {
    inner: ConfigFileInner,
    domain: String,
    project_path: String,
}

pub fn env_token(domain: &str) -> Result<String> {
    let env_domain = env_var(domain);
    Ok(std::env::var(format!("{}_API_TOKEN", env_domain))?)
}

fn env_var(domain: &str) -> String {
    let domain_fields = domain.split('.').collect::<Vec<&str>>();
    let env_domain = if domain_fields.len() == 1 {
        // There's not top level domain, such as .com
        domain
    } else {
        &domain_fields[0..domain_fields.len() - 1].join("_")
    };
    env_domain.to_ascii_uppercase()
}

impl ConfigFile {
    // TODO: make use of a BufReader instead
    /// Reads the configuration file and returns a ConfigFile struct that holds
    /// the configuration data for a given domain and project path.
    /// domain can be a top level domain such as gitlab.com or a subdomain such
    /// as gitlab.company.com.
    /// The project path is the path of the project in the remote after the domain.
    /// Ex: gitlab.com/jordilin/gitar -> /jordilin/gitar
    /// This is to allow for overriding project specific configurations such as
    /// reviewers, assignees, etc.
    pub fn new<T: Read, FE: Fn(&str) -> Result<String>>(
        mut reader: T,
        domain: &str,
        project_path: &str,
        env: FE,
    ) -> Result<ConfigFile> {
        let mut config_data = String::new();
        reader.read_to_string(&mut config_data)?;
        let mut config: ConfigFileInner = toml::from_str(&config_data)?;

        // ENV VAR API token takes preference. For a given domain, we try to fetch
        // <DOMAIN>_API_TOKEN env var first, then we fallback to the config
        // file. Given a domain such as gitlab.com, the env var to be set is
        // GITLAB_API_TOKEN. If the domain is gitlab.<company>.com, the env var
        // to be set is GITLAB_<COMPANY>_API_TOKEN.

        let domain_key = domain.replace('.', "_");
        if let Some(domain_config) = config.domains.get_mut(&domain_key) {
            if domain_config.api_token.is_none() {
                domain_config.api_token = Some(env(domain).map_err(|_| {
                    GRError::PreconditionNotMet(format!(
                        "No api_token found for domain {} in config or environment variable",
                        domain
                    ))
                })?);
            }
            Ok(ConfigFile {
                inner: config,
                domain: domain_key,
                project_path: project_path.to_string(),
            })
        } else {
            Err(error::gen(format!(
                "No config data found for domain {}",
                domain
            )))
        }
    }

    fn get_members_from_config(&self) -> Option<Vec<Member>> {
        if let Some(domain_config) = &self.inner.domains.get(&self.domain) {
            domain_config
                .projects
                .get(&self.project_path)
                .and_then(|project_config| {
                    project_config
                        .merge_requests
                        .as_ref()
                        .and_then(|merge_request_config| self.get_members(merge_request_config))
                })
                .or_else(|| {
                    domain_config
                        .merge_requests
                        .as_ref()
                        .and_then(|merge_request_config| self.get_members(merge_request_config))
                })
        } else {
            None
        }
    }

    fn get_members(&self, merge_request_config: &MergeRequestConfig) -> Option<Vec<Member>> {
        merge_request_config.members.as_ref().map(|users| {
            users
                .iter()
                .map(|user_info| match user_info {
                    UserInfo::UsernameOnly(username) => Member::builder()
                        .username(username.clone())
                        .build()
                        .unwrap(),
                    UserInfo::UsernameID { username, id } => Member::builder()
                        .username(username.clone())
                        .id(*id as i64)
                        .build()
                        .unwrap(),
                })
                .collect()
        })
    }
}

impl ConfigProperties for ConfigFile {
    fn api_token(&self) -> &str {
        if let Some(domain) = self.inner.domains.get(&self.domain) {
            domain.api_token.as_deref().unwrap_or_default()
        } else {
            ""
        }
    }

    fn cache_location(&self) -> Option<&str> {
        if let Some(domain) = self.inner.domains.get(&self.domain) {
            domain.cache_location.as_deref()
        } else {
            None
        }
    }

    fn preferred_assignee_username(&self) -> Option<Member> {
        if let Some(domain_config) = &self.inner.domains.get(&self.domain) {
            domain_config
                .projects
                .get(&self.project_path)
                .and_then(|project_config| {
                    project_config
                        .merge_requests
                        .as_ref()
                        .and_then(|merge_request_config| {
                            merge_request_config
                                .preferred_assignee_username
                                .as_ref()
                                .map(|user_info| match user_info {
                                    UserInfo::UsernameOnly(username) => Member::builder()
                                        .username(username.clone())
                                        .build()
                                        .unwrap(),
                                    UserInfo::UsernameID { username, id } => Member::builder()
                                        .username(username.clone())
                                        .id(*id as i64)
                                        .build()
                                        .unwrap(),
                                })
                        })
                })
                .or_else(|| {
                    domain_config
                        .merge_requests
                        .as_ref()
                        .and_then(|merge_request_config| {
                            merge_request_config
                                .preferred_assignee_username
                                .as_ref()
                                .map(|user_info| match user_info {
                                    UserInfo::UsernameOnly(username) => Member::builder()
                                        .username(username.clone())
                                        .build()
                                        .unwrap(),
                                    UserInfo::UsernameID { username, id } => Member::builder()
                                        .username(username.clone())
                                        .id(*id as i64)
                                        .build()
                                        .unwrap(),
                                })
                        })
                })
        } else {
            None
        }
    }

    fn merge_request_members(&self) -> Option<Vec<Member>> {
        self.get_members_from_config()
    }

    fn merge_request_description_signature(&self) -> &str {
        if let Some(domain_config) = &self.inner.domains.get(&self.domain) {
            domain_config
                .projects
                .get(&self.project_path)
                .and_then(|project_config| {
                    project_config
                        .merge_requests
                        .as_ref()
                        .and_then(|merge_request_config| {
                            merge_request_config.description_signature.as_deref()
                        })
                })
                .unwrap_or_else(|| {
                    domain_config
                        .merge_requests
                        .as_ref()
                        .and_then(|merge_request_config| {
                            merge_request_config.description_signature.as_deref()
                        })
                        .unwrap_or_default()
                })
        } else {
            ""
        }
    }

    fn get_cache_expiration(&self, api_operation: &ApiOperation) -> &str {
        self.inner
            .domains
            .get(&self.domain)
            .and_then(|domain_config| {
                domain_config
                    .cache_expirations
                    .as_ref()
                    .and_then(|cache_expirations| cache_expirations.settings.get(api_operation))
            })
            .map(|s| s.as_str())
            .unwrap_or_else(|| EXPIRE_IMMEDIATELY)
    }

    fn get_max_pages(&self, api_operation: &ApiOperation) -> u32 {
        self.inner
            .domains
            .get(&self.domain)
            .and_then(|domain_config| {
                domain_config
                    .max_pages_api
                    .as_ref()
                    .and_then(|max_pages| max_pages.settings.get(api_operation))
            })
            .copied()
            .unwrap_or(REST_API_MAX_PAGES)
    }

    fn rate_limit_remaining_threshold(&self) -> u32 {
        self.inner
            .domains
            .get(&self.domain)
            .and_then(|domain_config| domain_config.rate_limit_remaining_threshold)
            .unwrap_or(RATE_LIMIT_REMAINING_THRESHOLD)
    }
}

impl ConfigProperties for Arc<ConfigFile> {
    fn api_token(&self) -> &str {
        self.as_ref().api_token()
    }

    fn cache_location(&self) -> Option<&str> {
        self.as_ref().cache_location()
    }

    fn preferred_assignee_username(&self) -> Option<Member> {
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

    fn merge_request_members(&self) -> Option<Vec<Member>> {
        self.as_ref().merge_request_members()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn no_env(_: &str) -> Result<String> {
        Err(error::gen("No env var"))
    }

    #[test]
    fn test_config_ok() {
        let config_data = r#"
        [gitlab_com]
        api_token = '1234'
        cache_location = "/home/user/.config/mr_cache"
        rate_limit_remaining_threshold=15

        [gitlab_com.merge_requests]
        preferred_assignee_username = "jordilin"
        description_signature = "- devops team :-)"
        members = [
            { username = 'jdoe', id = 1231 },
            { username = 'jane', id = 1232 }
        ]

        [gitlab_com.max_pages_api]
        merge_request = 2
        pipeline = 3
        project = 4
        container_registry = 5
        single_page = 6
        release = 7
        gist = 8
        repository_tag = 9

        [gitlab_com.cache_expirations]
        merge_request = "30m"
        pipeline = "0s"
        project = "90d"
        container_registry = "0s"
        single_page = "0s"
        release = "4h"
        gist = "1w"
        repository_tag = "0s"
        "#;
        let domain = "gitlab.com";
        let reader = std::io::Cursor::new(config_data);
        let project_path = "/jordilin/gitar";
        let config = Arc::new(ConfigFile::new(reader, domain, project_path, no_env).unwrap());
        assert_eq!("1234", config.api_token());
        assert_eq!(
            "/home/user/.config/mr_cache",
            config.cache_location().unwrap()
        );
        assert_eq!(15, config.rate_limit_remaining_threshold());
        assert_eq!(
            "- devops team :-)",
            config.merge_request_description_signature()
        );
        let preferred_assignee_user = config.preferred_assignee_username().unwrap();
        assert_eq!("jordilin", preferred_assignee_user.username);
        assert_eq!(2, config.get_max_pages(&ApiOperation::MergeRequest));
        assert_eq!(3, config.get_max_pages(&ApiOperation::Pipeline));
        assert_eq!(4, config.get_max_pages(&ApiOperation::Project));
        assert_eq!(5, config.get_max_pages(&ApiOperation::ContainerRegistry));
        assert_eq!(6, config.get_max_pages(&ApiOperation::SinglePage));
        assert_eq!(7, config.get_max_pages(&ApiOperation::Release));
        assert_eq!(8, config.get_max_pages(&ApiOperation::Gist));
        assert_eq!(9, config.get_max_pages(&ApiOperation::RepositoryTag));

        assert_eq!(
            "30m",
            config.get_cache_expiration(&ApiOperation::MergeRequest)
        );
        assert_eq!("0s", config.get_cache_expiration(&ApiOperation::Pipeline));
        assert_eq!("90d", config.get_cache_expiration(&ApiOperation::Project));
        assert_eq!(
            "0s",
            config.get_cache_expiration(&ApiOperation::ContainerRegistry)
        );
        assert_eq!("0s", config.get_cache_expiration(&ApiOperation::SinglePage));
        assert_eq!("4h", config.get_cache_expiration(&ApiOperation::Release));
        assert_eq!("1w", config.get_cache_expiration(&ApiOperation::Gist));
        assert_eq!(
            "0s",
            config.get_cache_expiration(&ApiOperation::RepositoryTag)
        );
        let members = config.merge_request_members().unwrap();
        assert_eq!(2, members.len());
        assert_eq!("jdoe", members[0].username);
        assert_eq!(1231, members[0].id);
        assert_eq!("jane", members[1].username);
        assert_eq!(1232, members[1].id);
    }

    #[test]
    fn test_config_defaults() {
        let config_data = r#"
        [github_com]
        api_token = '1234'
        "#;
        let domain = "github.com";
        let reader = std::io::Cursor::new(config_data);
        let project_path = "/jordilin/gitar";
        let config = Arc::new(ConfigFile::new(reader, domain, project_path, no_env).unwrap());
        for api_operation in ApiOperation::iter() {
            assert_eq!(REST_API_MAX_PAGES, config.get_max_pages(&api_operation));
            assert_eq!(
                EXPIRE_IMMEDIATELY,
                config.get_cache_expiration(&api_operation)
            );
        }
        assert_eq!(
            RATE_LIMIT_REMAINING_THRESHOLD,
            config.rate_limit_remaining_threshold()
        );
        assert_eq!(None, config.cache_location());
        assert_eq!(None, config.preferred_assignee_username());
        assert_eq!("", config.merge_request_description_signature());
    }

    #[test]
    fn test_config_with_overridden_project_specific_settings() {
        let config_data = r#"
        [gitlab_com]
        api_token = '1234'
        cache_location = "/home/user/.config/mr_cache"
        rate_limit_remaining_threshold=15

        [gitlab_com.merge_requests]
        preferred_assignee_username = "jordilin"
        description_signature = "- devops team :-)"
        members = [
            { username = 'jdoe', id = 1231 }
        ]

        # Project specific settings for /datateam/projecta
        [gitlab_com.datateam_projecta.merge_requests]
        preferred_assignee_username = 'jdoe'
        description_signature = '- data team projecta :-)'
        members = [ { username = 'jane', id = 1234 } ]"#;

        let domain = "gitlab.com";
        let reader = std::io::Cursor::new(config_data);
        let project_path = "datateam_projecta";
        let config = Arc::new(ConfigFile::new(reader, domain, project_path, no_env).unwrap());
        let preferred_assignee_user = config.preferred_assignee_username().unwrap();
        assert_eq!("jdoe", preferred_assignee_user.username);
        assert_eq!(
            "- data team projecta :-)",
            config.merge_request_description_signature()
        );
        let members = config.merge_request_members().unwrap();
        assert_eq!(1, members.len());
        assert_eq!("jane", members[0].username);
        assert_eq!(1234, members[0].id);
    }

    #[test]
    fn test_config_preferred_assignee_username_with_id() {
        let config_data = r#"
        [gitlab_com]
        api_token = '1234'
        cache_location = "/home/user/.config/mr_cache"
        rate_limit_remaining_threshold=15

        [gitlab_com.merge_requests]
        preferred_assignee_username = { username = 'jdoe', id = 1231 }

        # Project specific settings for /datateam/projecta
        [gitlab_com.datateam_projecta.merge_requests]
        preferred_assignee_username = { username = 'jordilin', id = 1234 }
        "#;

        let domain = "gitlab.com";
        let reader = std::io::Cursor::new(config_data);
        let project_path = "datateam_projecta";
        let config = Arc::new(ConfigFile::new(reader, domain, project_path, no_env).unwrap());
        let preferred_assignee_user = config.preferred_assignee_username().unwrap();
        assert_eq!("jordilin", preferred_assignee_user.username);
    }

    #[test]
    fn test_no_api_token_is_err() {
        let config_data = r#"
        [gitlab_com]
        api_token_typo=1234"#;
        let domain = "gitlab.com";
        let reader = std::io::Cursor::new(config_data);
        let project_path = "/jordilin/gitar";
        assert!(ConfigFile::new(reader, domain, project_path, no_env).is_err());
    }

    #[test]
    fn test_config_no_data() {
        let config_data = "";
        let domain = "gitlab.com";
        let reader = std::io::Cursor::new(config_data);
        let project_path = "/jordilin/gitar";
        assert!(ConfigFile::new(reader, domain, project_path, no_env).is_err());
    }

    fn env(_: &str) -> Result<String> {
        Ok("1234".to_string())
    }

    #[test]
    fn test_use_gitlab_com_api_token_envvar() {
        let config_data = r#"
        [gitlab_com]
        "#;
        let domain = "gitlab.com";
        let reader = std::io::Cursor::new(config_data);
        let project_path = "/jordilin/gitar";
        let config = Arc::new(ConfigFile::new(reader, domain, project_path, env).unwrap());
        assert_eq!("1234", config.api_token());
    }

    #[test]
    fn test_use_sub_domain_gitlab_token_env_var() {
        let config_data = r#"
        [gitlab_company_com]
        "#;
        let domain = "gitlab.company.com";
        let reader = std::io::Cursor::new(config_data);
        let project_path = "/jordilin/gitar";
        let config = Arc::new(ConfigFile::new(reader, domain, project_path, env).unwrap());
        assert_eq!("1234", config.api_token());
    }

    #[test]
    fn test_domain_without_top_level_domain_token_envvar() {
        let config_data = r#"
        [gitlabweb]
        "#;
        let domain = "gitlabweb";
        let reader = std::io::Cursor::new(config_data);
        let project_path = "/jordilin/gitar";
        let config = Arc::new(ConfigFile::new(reader, domain, project_path, env).unwrap());
        assert_eq!("1234", config.api_token());
    }

    #[test]
    fn test_no_config_requires_auth_env_token_and_no_cache() {
        let domain = "gitlabwebnoconfig";
        let config = NoConfig::new(domain, env).unwrap();
        assert_eq!("1234", config.api_token());
        assert_eq!(None, config.cache_location());
    }

    #[test]
    fn test_no_config_no_env_token_is_error() {
        let domain = "gitlabwebnoenv.com";
        let config_res = NoConfig::new(domain, no_env);
        match config_res {
            Err(err) => match err.downcast_ref::<error::GRError>() {
                Some(error::GRError::PreconditionNotMet(val)) => {
                    assert_eq!("Configuration not found, so it is expected environment variable GITLABWEBNOENV_API_TOKEN to be set.", val)
                }
                _ => panic!("Expected error::GRError::PreconditionNotMet"),
            },
            _ => panic!("Expected error"),
        }
    }

    #[test]
    fn test_default_config_file() {
        // This is the case when browsing and no configuration is needed.
        let config = ConfigFile::default();
        assert_eq!("", config.api_token());
        assert_eq!(None, config.cache_location());
        assert_eq!(
            RATE_LIMIT_REMAINING_THRESHOLD,
            config.rate_limit_remaining_threshold()
        );
        assert_eq!(None, config.preferred_assignee_username());
        assert_eq!("", config.merge_request_description_signature());
    }
}
