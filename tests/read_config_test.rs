use gr::remote::read_config;
use std::io::Write;
use std::path::Path;
use tempfile::NamedTempFile;

fn create_temp_config_file(content: &str) -> NamedTempFile {
    let mut temp_file = NamedTempFile::new().unwrap();
    write!(temp_file, "{}", content).unwrap();
    temp_file.flush().unwrap();
    temp_file
}

#[test]
fn test_no_config_auth_env_and_no_cache_flag_ok() {}

#[test]
fn test_read_config_valid() {
    let config_content = r#"
    github.com.api_token=1234
    github.com.cache_location=/tmp/cache
    gitlab.com.api_token=5678
    gitlab.com.cache_location=/tmp/cache2
    "#;
    let temp_file = create_temp_config_file(config_content);
    let result = read_config(temp_file.path(), "github.com");
    assert!(result.is_ok());
    let config = result.unwrap();
    assert_eq!(config.api_token(), "1234");
    assert_eq!(config.cache_location().unwrap(), "/tmp/cache");
}

#[test]
fn test_read_config_file_not_found_and_no_token_env_var_is_error() {
    let result = read_config(Path::new("/non/existent/path.txt"), "github.com");
    assert!(result.is_err());
}

#[test]
fn test_read_config_file_not_found_with_token_env_var_is_ok() {
    std::env::set_var("INTEGRATIONTEST_API_TOKEN", "123");
    let config_res = read_config(Path::new("/non/existent/path.txt"), "integrationtest.com");
    assert!(config_res.is_ok());
    std::env::remove_var("INTEGRATIONTEST_API_TOKEN");
}

#[test]
fn test_read_config_empty_file() {
    let temp_file = create_temp_config_file("");
    let result = read_config(temp_file.path(), "github.com");
    assert!(result.is_err());
}

#[test]
fn test_read_config_invalid_data() {
    let config_content = r#"
    github.com.api_token=1234
    github.com.cache_location
    "#;
    let temp_file = create_temp_config_file(config_content);
    let result = read_config(temp_file.path(), "github.com");
    assert!(result.is_err());
}

#[test]
fn test_read_config_unknown_domain() {
    let config_content = r#"
    github.com.api_token=1234
    github.com.cache_location=/tmp/cache
    "#;
    let temp_file = create_temp_config_file(config_content);
    let result = read_config(temp_file.path(), "gitlab.com");
    assert!(result.is_err());
}
