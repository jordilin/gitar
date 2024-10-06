use gr::cli::CliArgs;
use gr::remote::{read_config, ConfigFilePath, RemoteURL};

#[test]
fn test_read_config_valid() {
    let project_path = "/jordilin/gitar".to_string();
    let url = RemoteURL::new("github.test.com".to_string(), project_path);
    let cli_args = CliArgs::new(
        0,
        None,
        None,
        Some("./tests/fixtures/configs/ok".to_string()),
    );
    let config_path = ConfigFilePath::new(&cli_args);
    let result = read_config(config_path, &url);
    assert!(result.is_ok());
    let config = result.unwrap();
    assert_eq!(config.api_token(), "1234");
    assert_eq!(config.cache_location().unwrap(), "/tmp/cache");
}

#[test]
fn test_read_config_file_not_found_and_no_token_env_var_is_error() {
    let project_path = "/jordilin/gitar".to_string();
    let url = RemoteURL::new("github.integrationtest.com".to_string(), project_path);
    let cli_args = CliArgs::new(0, None, None, Some("/path/does/not/exist".to_string()));
    let config_path = ConfigFilePath::new(&cli_args);
    let result = read_config(config_path, &url);
    assert!(result.is_err());
}

#[test]
fn test_read_config_file_not_found_with_token_env_var_is_ok() {
    std::env::set_var("INTEGRATIONTEST_API_TOKEN", "123");
    let project_path = "/jordilin/gitar".to_string();
    let url = RemoteURL::new("integrationtest.com".to_string(), project_path);
    let cli_args = CliArgs::new(0, None, None, Some("/path/does/not/exist".to_string()));
    let config_path = ConfigFilePath::new(&cli_args);
    let config_res = read_config(config_path, &url);
    assert!(config_res.is_ok());
    std::env::remove_var("INTEGRATIONTEST_API_TOKEN");
}

#[test]
fn test_read_config_empty_file() {
    let project_path = "/jordilin/gitar".to_string();
    let url = RemoteURL::new("github.com".to_string(), project_path);
    let cli_args = CliArgs::new(
        0,
        None,
        None,
        Some("./tests/fixtures/configs/ok_empty".to_string()),
    );
    let config_path = ConfigFilePath::new(&cli_args);
    let result = read_config(config_path, &url);
    assert!(result.is_err());
}

#[test]
fn test_read_config_invalid_toml_data() {
    let project_path = "/jordilin/gitar".to_string();
    let cli_args = CliArgs::new(
        0,
        None,
        None,
        Some("./tests/fixtures/configs/invalid_toml".to_string()),
    );
    let config_path = ConfigFilePath::new(&cli_args);
    let url = RemoteURL::new("github.com".to_string(), project_path);
    assert!(read_config(config_path, &url).is_err());
}

#[test]
fn test_read_config_unknown_domain() {
    let project_path = "/jordilin/gitar".to_string();
    let url = RemoteURL::new("gitlab.com".to_string(), project_path);
    let cli_args = CliArgs::new(
        0,
        None,
        None,
        Some("./tests/fixtures/configs/invalid_domain".to_string()),
    );
    let config_path = ConfigFilePath::new(&cli_args);
    let result = read_config(config_path, &url);
    assert!(result.is_err());
}
