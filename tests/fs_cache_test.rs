use gr::error::GRError;
use std::path::PathBuf;
use std::{fs, sync::Arc};
use tempfile::TempDir;

use gr::{
    api_traits::ApiOperation,
    cache::{filesystem::FileCache, Cache, CacheState},
    config::ConfigProperties,
    http::{Headers, Resource},
    io::{HttpResponse, ResponseField},
};

struct TestConfig {
    cache_dir: PathBuf,
}

impl ConfigProperties for TestConfig {
    fn api_token(&self) -> &str {
        "test_token"
    }

    fn cache_location(&self) -> Option<&str> {
        Some(self.cache_dir.to_str().unwrap())
    }

    fn get_cache_expiration(&self, _: &ApiOperation) -> &str {
        "3600s"
    }
}

#[test]
fn test_file_cache_fresh() {
    // Create a temporary directory for our cache
    let temp_dir = TempDir::new().unwrap();
    let config = TestConfig {
        cache_dir: temp_dir.path().to_path_buf(),
    };

    let file_cache = FileCache::new(Arc::new(config));

    let resource = Resource::new("https://api.example.com/test", Some(ApiOperation::Project));

    let response = HttpResponse::builder()
        .status(200)
        .body("Test response body".to_string())
        .headers({
            let mut h = Headers::new();
            h.set(
                "cache-control".to_string(),
                "max-age=7200, no-cache".to_string(),
            );
            h
        })
        .build()
        .unwrap();

    file_cache.set(&resource, &response).unwrap();

    // Verify the cache file was created
    let cache_file = file_cache.get_cache_file(&resource.url);
    assert!(fs::metadata(&cache_file).is_ok());

    // Test getting a fresh value from the cache
    match file_cache.get(&resource).unwrap() {
        CacheState::Fresh(cached_response) => {
            assert_eq!(cached_response.status, 200);
            assert_eq!(cached_response.body, "Test response body");
            assert_eq!(
                cached_response
                    .headers
                    .unwrap()
                    .get("cache-control")
                    .unwrap(),
                "max-age=7200, no-cache"
            );
        }
        _ => panic!("Expected a fresh cache state"),
    }
}

#[test]
fn test_file_cache_stale_user_expired_no_cache() {
    let temp_dir = TempDir::new().unwrap();
    let config = TestConfig {
        cache_dir: temp_dir.path().to_path_buf(),
    };

    let file_cache = FileCache::new(Arc::new(config));

    let resource = Resource::new("https://api.example.com/test", Some(ApiOperation::Project));

    let response = HttpResponse::builder()
        .status(200)
        .body("Test response body".to_string())
        .headers({
            let mut h = Headers::new();
            h.set(
                "cache-control".to_string(),
                "max-age=7200, no-cache".to_string(),
            );
            h
        })
        .build()
        .unwrap();

    file_cache.set(&resource, &response).unwrap();

    let cache_file = file_cache.get_cache_file(&resource.url);

    // Simulate passage of time (beyond user-defined expiration)
    let cache_file_path = PathBuf::from(cache_file);
    let metadata = fs::metadata(&cache_file_path).unwrap();
    let mtime = metadata.modified().unwrap() - std::time::Duration::from_secs(4000);
    filetime::set_file_mtime(&cache_file_path, filetime::FileTime::from(mtime)).unwrap();

    // Test getting a stale value from the cache (due to user expiration). Cache
    // control contains no-cache directive, so the cache should be considered stale.
    match file_cache.get(&resource).unwrap() {
        CacheState::Stale(cached_response) => {
            assert_eq!(cached_response.status, 200);
            assert_eq!(cached_response.body, "Test response body");
        }
        _ => panic!("Expected a stale cache state"),
    }
}

#[test]
fn test_file_cache_fresh_user_expired_but_under_max_age() {
    let temp_dir = TempDir::new().unwrap();
    let config = TestConfig {
        cache_dir: temp_dir.path().to_path_buf(),
    };

    let file_cache = FileCache::new(Arc::new(config));

    let resource = Resource::new("https://api.example.com/test", Some(ApiOperation::Project));

    let response = HttpResponse::builder()
        .status(200)
        .body("Test response body".to_string())
        .headers({
            let mut h = Headers::new();
            h.set("cache-control".to_string(), "max-age=7200".to_string());
            h
        })
        .build()
        .unwrap();

    file_cache.set(&resource, &response).unwrap();

    let cache_file = file_cache.get_cache_file(&resource.url);

    // Simulate passage of time (beyond user-defined expiration but within HTTP cache control)
    let cache_file_path = PathBuf::from(cache_file);
    let metadata = fs::metadata(&cache_file_path).unwrap();
    let mtime = metadata.modified().unwrap() - std::time::Duration::from_secs(4000);
    filetime::set_file_mtime(&cache_file_path, filetime::FileTime::from(mtime)).unwrap();

    match file_cache.get(&resource).unwrap() {
        CacheState::Fresh(cached_response) => {
            assert_eq!(cached_response.status, 200);
            assert_eq!(cached_response.body, "Test response body");
            assert_eq!(
                cached_response
                    .headers
                    .unwrap()
                    .get("cache-control")
                    .unwrap(),
                "max-age=7200"
            );
        }
        _ => panic!("Expected a fresh cache state"),
    }
}

#[test]
fn test_cache_stale_both_user_expired_and_max_aged_expired() {
    let temp_dir = TempDir::new().unwrap();
    let config = TestConfig {
        cache_dir: temp_dir.path().to_path_buf(),
    };

    let file_cache = FileCache::new(Arc::new(config));

    let resource = Resource::new("https://api.example.com/test", Some(ApiOperation::Project));

    let response = HttpResponse::builder()
        .status(200)
        .body("Test response body".to_string())
        .headers({
            let mut h = Headers::new();
            h.set("cache-control".to_string(), "max-age=7200".to_string());
            h
        })
        .build()
        .unwrap();

    file_cache.set(&resource, &response).unwrap();

    let cache_file = file_cache.get_cache_file(&resource.url);
    // Simulate passage of time (beyond both user-defined and HTTP cache control
    // expiration)
    let cache_file_path = PathBuf::from(cache_file);
    let metadata = fs::metadata(&cache_file_path).unwrap();
    let mtime = metadata.modified().unwrap() - std::time::Duration::from_secs(8000);
    filetime::set_file_mtime(cache_file_path, filetime::FileTime::from(mtime)).unwrap();

    // Test getting a stale value from the cache (due to both expirations)
    match file_cache.get(&resource).unwrap() {
        CacheState::Stale(cached_response) => {
            assert_eq!(cached_response.status, 200);
            assert_eq!(cached_response.body, "Test response body");
        }
        _ => panic!("Expected a stale cache state"),
    }
}

#[test]
fn test_cache_update_refreshes_cache() {
    let temp_dir = TempDir::new().unwrap();
    let config = TestConfig {
        cache_dir: temp_dir.path().to_path_buf(),
    };

    let file_cache = FileCache::new(Arc::new(config));

    let resource = Resource::new("https://api.example.com/test", Some(ApiOperation::Project));

    // First response
    let response = HttpResponse::builder()
        .status(200)
        .body("Test response body".to_string())
        .headers({
            let mut h = Headers::new();
            h.set("cache-control".to_string(), "max-age=7200".to_string());
            h
        })
        .build()
        .unwrap();

    file_cache.set(&resource, &response).unwrap();

    // Second call, we get a 304 response
    let new_response = HttpResponse::builder()
        .status(304)
        .headers(Headers::new())
        .build()
        .unwrap();

    // Update the cache with new headers
    file_cache
        .update(&resource, &new_response, &ResponseField::Headers)
        .unwrap();

    // Cache gets refreshed, so we should get a fresh cache state
    match file_cache.get(&resource).unwrap() {
        CacheState::Fresh(cached_response) => {
            assert_eq!(cached_response.status, 200);
            assert_eq!(cached_response.body, "Test response body");
        }
        _ => panic!("Expected a fresh cache state"),
    }
}

#[test]
fn test_validate_cache_location_success() {
    let temp_dir = TempDir::new().unwrap();
    let config = TestConfig {
        cache_dir: temp_dir.path().to_path_buf(),
    };

    let file_cache = FileCache::new(Arc::new(config));
    assert!(file_cache.validate_cache_location().is_ok());
}

#[test]
fn test_validate_cache_location_not_found() {
    let config = TestConfig {
        cache_dir: PathBuf::from("/non/existent/directory"),
    };

    let file_cache = FileCache::new(Arc::new(config));
    let err = file_cache.validate_cache_location().unwrap_err();
    match err.downcast_ref::<GRError>() {
        Some(GRError::CacheLocationDoesNotExist(msg)) => {
            assert!(msg.contains("/non/existent/directory"));
        }
        _ => panic!("Expected CacheLocationDoesNotExist error"),
    }
}

#[test]
fn test_validate_cache_location_not_a_directory() {
    let temp_dir = TempDir::new().unwrap();
    let temp_file = temp_dir.path().join("not_a_directory");
    fs::write(&temp_file, "").unwrap();

    let config = TestConfig {
        cache_dir: temp_file.clone(),
    };

    let file_cache = FileCache::new(Arc::new(config));
    let err = file_cache.validate_cache_location().unwrap_err();
    match err.downcast_ref::<GRError>() {
        Some(GRError::CacheLocationIsNotADirectory(msg)) => {
            assert!(msg.contains(temp_file.to_string_lossy().as_ref()));
        }
        _ => panic!("Expected CacheLocationIsNotADirectory error"),
    }
}

#[test]
fn test_validate_cache_location_not_writable() {
    let temp_dir = TempDir::new().unwrap();
    let cache_dir = temp_dir.path().to_path_buf();

    // Make the directory read-only
    let mut perms = fs::metadata(&cache_dir).unwrap().permissions();
    perms.set_readonly(true);
    fs::set_permissions(&cache_dir, perms).unwrap();

    let config = TestConfig {
        cache_dir: cache_dir.clone(),
    };

    let file_cache = FileCache::new(Arc::new(config));
    let err = file_cache.validate_cache_location().unwrap_err();
    match err.downcast_ref::<GRError>() {
        Some(GRError::CacheLocationIsNotWriteable(msg)) => {
            assert!(msg.contains(cache_dir.to_string_lossy().as_ref()));
        }
        _ => panic!("Expected CacheLocationIsNotWriteable error"),
    }

    // Restore permissions for cleanup
    let mut perms = fs::metadata(&cache_dir).unwrap().permissions();
    perms.set_readonly(false);
    fs::set_permissions(&cache_dir, perms).unwrap();
}

#[test]
fn test_validate_cache_location_config_not_found() {
    struct NoConfig;

    impl ConfigProperties for NoConfig {
        fn api_token(&self) -> &str {
            "test_token"
        }

        fn cache_location(&self) -> Option<&str> {
            None
        }

        fn get_cache_expiration(&self, _: &ApiOperation) -> &str {
            "3600s"
        }
    }

    let file_cache = FileCache::new(Arc::new(NoConfig));
    let err = file_cache.validate_cache_location().unwrap_err();
    match err.downcast_ref::<GRError>() {
        Some(GRError::ConfigurationNotFound) => {}
        _ => panic!("Expected ConfigurationNotFound error"),
    }
}
