use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

use gr::{
    api_traits::ApiOperation,
    cache::{filesystem::FileCache, Cache, CacheState},
    config::ConfigProperties,
    http::{Headers, Resource},
    io::{Response, ResponseField},
};

struct TestConfig {
    cache_dir: PathBuf,
}

impl ConfigProperties for TestConfig {
    fn api_token(&self) -> &str {
        "test_token"
    }

    fn cache_location(&self) -> &str {
        self.cache_dir.to_str().unwrap()
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

    let file_cache = FileCache::new(config);

    let resource = Resource::new("https://api.example.com/test", Some(ApiOperation::Project));

    let response = Response::builder()
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

    let file_cache = FileCache::new(config);

    let resource = Resource::new("https://api.example.com/test", Some(ApiOperation::Project));

    let response = Response::builder()
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

    let file_cache = FileCache::new(config);

    let resource = Resource::new("https://api.example.com/test", Some(ApiOperation::Project));

    let response = Response::builder()
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

    let file_cache = FileCache::new(config);

    let resource = Resource::new("https://api.example.com/test", Some(ApiOperation::Project));

    let response = Response::builder()
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

    let file_cache = FileCache::new(config);

    let resource = Resource::new("https://api.example.com/test", Some(ApiOperation::Project));

    // First response
    let response = Response::builder()
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
    let new_response = Response::builder()
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
