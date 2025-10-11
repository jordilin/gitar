use std::sync::Arc;

use gr::cache::{Cache, InMemoryCache, NoCache};
use gr::config::ConfigProperties;
use gr::error::GRError;
use gr::http::{Client, Headers, Method, Request};
use gr::io::{HttpResponse, HttpRunner, ResponseField};
use httpmock::prelude::*;
use httpmock::Method::{GET, HEAD, PATCH, POST};

struct ConfigMock {}

impl ConfigMock {
    fn new() -> Self {
        ConfigMock {}
    }
}

impl ConfigProperties for ConfigMock {
    fn api_token(&self) -> &str {
        "1234"
    }
    fn cache_location(&self) -> Option<&str> {
        Some("")
    }
}

#[test]
fn test_http_runner() {
    let server = MockServer::start();
    let body_str = r#"
    {
        "id": 4,
        "default_branch": "main",
    }"#;
    let server_mock = server.mock(|when, then| {
        when.method(GET).path("/repos/jordilin/mr");
        then.status(200)
            .header("content-type", "application/json")
            .body(body_str);
    });

    let runner = Client::new(NoCache, Arc::new(ConfigMock::new()), false);
    let mut request = Request::<()>::new(&server.url("/repos/jordilin/mr"), Method::GET);
    let response = runner.run(&mut request).unwrap();
    assert_eq!(response.status, 200);
    assert!(response.body.contains("id"));
    server_mock.assert();
}

#[test]
fn test_http_runner_head_request() {
    let server = MockServer::start();
    let server_mock = server.mock(|when, then| {
        when.method(HEAD).path("/repos/jordilin/mr");
        then.status(200)
            .header("content-type", "application/json")
            .header("link", "<https://api.github.com/repositories/683565078/pulls?state=closed&page=2>; rel=\"next\", <https://api.github.com/repositories/683565078/pulls?state=closed&page=5>; rel=\"last\"");
    });

    let runner = Client::new(NoCache, Arc::new(ConfigMock::new()), false);
    let mut request = Request::<()>::new(&server.url("/repos/jordilin/mr"), Method::HEAD);
    let response = runner.run(&mut request).unwrap();
    assert_eq!(response.status, 200);
    assert_eq!(response.header("link"), Some("<https://api.github.com/repositories/683565078/pulls?state=closed&page=2>; rel=\"next\", <https://api.github.com/repositories/683565078/pulls?state=closed&page=5>; rel=\"last\""));
    server_mock.assert();
}

#[test]
fn test_http_runner_server_down_get_request() {
    let runner = Client::new(NoCache, Arc::new(ConfigMock::new()), false);
    let mut request = Request::<()>::new("http://localhost:8091/repos/jordilin/mr", Method::GET);
    let err = runner.run(&mut request).unwrap_err();
    match err.downcast_ref::<GRError>() {
        Some(GRError::HttpTransportError(_)) => (),
        _ => panic!("Expected GRError::HttpTransportError, but got {err:?}"),
    }
}

#[test]
fn test_http_runner_server_down_post_request() {
    let runner = Client::new(NoCache, Arc::new(ConfigMock::new()), false);
    let mut request = Request::<()>::new("http://localhost:8091/repos/jordilin/mr", Method::POST);
    let err = runner.run(&mut request).unwrap_err();
    match err.downcast_ref::<GRError>() {
        Some(GRError::HttpTransportError(_)) => (),
        _ => panic!("Expected GRError::HttpTransportError, but got {err:?}"),
    }
}

#[test]
fn test_http_runner_post_request() {
    let server = MockServer::start();
    let body_str = r#"
    {
        "id": 4,
        "default_branch": "main",
    }"#;
    let server_mock = server.mock(|when, then| {
        when.method(POST).path("/repos/jordilin/mr");
        then.status(201)
            .header("content-type", "application/json")
            .body(body_str);
    });

    let runner = Client::new(NoCache, Arc::new(ConfigMock::new()), false);
    let mut request = Request::<()>::new(&server.url("/repos/jordilin/mr"), Method::POST);
    let response = runner.run(&mut request).unwrap();
    assert_eq!(response.status, 201);
    assert!(response.body.contains("id"));
    server_mock.assert();
}

#[test]
fn test_http_gathers_from_inmemory_fresh_cache() {
    let server = MockServer::start();
    let body_str = r#"
    {
        "id": 4,
        "default_branch": "main",
    }"#;

    let response = HttpResponse::builder()
        .status(200)
        .body(body_str.to_string())
        .build()
        .unwrap();

    // We setup the mock expectations, but we will make sure it was never hit as
    // we will make use of an inmemory cache.
    let server_mock = server.mock({
        |when, then| {
            when.method(GET).path("/repos/jordilin/mr");
            then.status(200)
                .header("content-type", "application/json")
                .body(body_str);
        }
    });
    // This request is cacheable with an inmemory cache
    let cache = &InMemoryCache::default();
    let url = format!("{}/repos/jordilin/mr", server.address());
    cache.set(&url, &response).unwrap();

    // Set up the http client with an inmemory cache
    let runner = Client::new(cache, Arc::new(ConfigMock::new()), false);
    let mut request = Request::<()>::new(&url, Method::GET);

    // Execute the client
    let response = runner.run(&mut request).unwrap();
    assert_eq!(response.status, 200);
    assert!(response.body.contains("id"));

    assert!(response.local_cache);

    // Mock was never called. We used the cache
    server_mock.assert_calls(0);
}

#[test]
fn test_http_gathers_from_inmemory_stale_cache_server_304() {
    let server = MockServer::start();

    // Server returns no modified 304 status with no content and expects to
    // receive the If-None-Match header in the request.
    let server_mock = server.mock({
        |when, then| {
            when.method(GET)
                .header_exists("If-None-Match")
                .path("/repos/jordilin/mr/members");
            then.status(304)
                .header("content-type", "application/json")
                .header("content-encoding", "gzip")
                .body("");
        }
    });

    let body_str = r#"
    {
        "id": 4,
        "default_branch": "main",
    }"#;

    let mut headers = Headers::new();
    headers.set("etag".to_string(), "1234".to_string());
    headers.set("Max-Age".to_string(), "0".to_string());
    let response = HttpResponse::builder()
        .status(200)
        .body(body_str.to_string())
        .headers(headers)
        .build()
        .unwrap();
    let url = format!("http://{}/repos/jordilin/mr/members", server.address());
    let mut cache = InMemoryCache::default();
    cache.set(&url, &response).unwrap();
    cache.expire();

    let runner = Client::new(&cache, Arc::new(ConfigMock::new()), false);
    let mut request = Request::<()>::new(&url, Method::GET);

    let response = runner.run(&mut request).unwrap();
    assert_eq!(response.status, 200);
    assert!(!response.local_cache);
    assert!(response.body.contains("id"));

    // While do we have a cache, the cache was expired, hence we expect the
    // server to be hit with a 304 status and a If-None-Match header set in the
    // request.
    server_mock.assert_calls(1);
    // 304 - cache has been updated with the new upstream headers
    assert!(*cache.updated.borrow());
    assert_eq!(ResponseField::Headers, *cache.updated_field.borrow(),);
}

#[test]
fn test_http_get_hits_endpoint_use_cache_on_second_call() {
    let server = MockServer::start();
    let body_str = r#"
    {
        "id": 4,
        "default_branch": "main",
    }"#;

    // We setup the mock expectations, but we will make sure it was never hit as
    // we will make use of an inmemory cache.
    let server_mock = server.mock({
        |when, then| {
            when.method(GET).path("/repos/jordilin/mr");
            then.status(200)
                .header("content-type", "application/json")
                .body(body_str);
        }
    });

    let cache = &InMemoryCache::default();
    let url = format!("http://{}/repos/jordilin/mr", server.address());
    // call is not cached yet using URL as key

    // Set up the http client with an inmemory cache
    let runner = Client::new(cache, Arc::new(ConfigMock::new()), false);
    let mut request = Request::<()>::new(&url, Method::GET);

    // Execute the client
    let response = runner.run(&mut request).unwrap();
    assert_eq!(response.status, 200);
    assert!(response.body.contains("id"));

    // Verify the endpoint was hit
    server_mock.assert_calls(1);

    // Call is cached now
    // Do a second request and verify that the mock was not called
    let mut request = Request::<()>::new(&url, Method::GET);

    // Execute the client
    let response = runner.run(&mut request).unwrap();
    assert_eq!(response.status, 200);
    assert!(response.body.contains("id"));

    // Number of calls received is still 1 from the previous call
    server_mock.assert_calls(1);
}

#[test]
fn test_http_post_hits_endpoint_two_times_does_not_use_cache() {
    let server = MockServer::start();
    let body_str = r#"
    {
        "id": 4,
        "default_branch": "main",
    }"#;

    // We setup the mock expectations, but we will make sure it was never hit as
    // we will make use of an inmemory cache.
    let server_mock = server.mock({
        |when, then| {
            when.method(POST).path("/repos/jordilin/mr");
            then.status(200)
                .header("content-type", "application/json")
                .body(body_str);
        }
    });

    let cache = &InMemoryCache::default();
    let url = format!("http://{}/repos/jordilin/mr", server.address());
    // call is not cached yet using URL as key

    // Set up the http client with an inmemory cache
    let runner = Client::new(cache, Arc::new(ConfigMock::new()), false);
    let mut request = Request::<()>::new(&url, Method::POST);

    // Execute the client
    let response = runner.run(&mut request).unwrap();
    assert_eq!(response.status, 200);
    assert!(response.body.contains("id"));

    // Verify the endpoint was hit
    server_mock.assert_calls(1);

    // Call is cached now
    // Do a second request and verify that the mock was not called
    let mut request = Request::<()>::new(&url, Method::POST);

    // Execute the client
    let response = runner.run(&mut request).unwrap();
    assert_eq!(response.status, 200);
    assert!(response.body.contains("id"));

    // Number of calls received is still 1 from the previous call
    server_mock.assert_calls(2);
}

#[test]
fn test_http_runner_patch_request() {
    let server = MockServer::start();
    let body_str = r#"
    {
        "username": "jordilin",
    }"#;
    let server_mock = server.mock(|when, then| {
        when.method(PATCH).path("/repos/jordilin/mr");
        then.status(200)
            .header("content-type", "application/json")
            .body(body_str);
    });

    let runner = Client::new(NoCache, Arc::new(ConfigMock::new()), false);
    let mut request = Request::<()>::new(&server.url("/repos/jordilin/mr"), Method::PATCH);
    let response = runner.run(&mut request).unwrap();
    assert_eq!(response.status, 200);
    assert!(response.body.contains("username"));
    server_mock.assert();
}

#[test]
fn test_http_get_hits_endpoint_dont_use_cache_if_refresh_cache_is_set() {
    let server = MockServer::start();
    let body_str = r#"
    {
        "id": 4,
        "default_branch": "main",
    }"#;

    // We setup the mock expectations, but we will make sure it was never hit as
    // we will make use of an inmemory cache.
    let server_mock = server.mock({
        |when, then| {
            when.method(GET).path("/repos/jordilin/mr");
            then.status(200)
                .header("content-type", "application/json")
                .body(body_str);
        }
    });

    let cache = &InMemoryCache::default();
    let url = format!("http://{}/repos/jordilin/mr", server.address());
    // call is not cached yet using URL as key

    // Set up the http client with an inmemory cache and refresh cache is set.
    let runner = Client::new(cache, Arc::new(ConfigMock::new()), true);
    let mut request = Request::<()>::new(&url, Method::GET);

    // Execute the client
    let response = runner.run(&mut request).unwrap();
    assert_eq!(response.status, 200);
    assert!(response.body.contains("id"));

    // Verify the endpoint was hit
    server_mock.assert_calls(1);

    // Call is cached now
    // Do a second request and verify that the mock was not called
    let mut request = Request::<()>::new(&url, Method::GET);

    // Execute the client
    let response = runner.run(&mut request).unwrap();
    assert_eq!(response.status, 200);
    assert!(response.body.contains("id"));

    // We enforce cache refreshment, so the number of calls received by the HTTP
    // server is 2.
    server_mock.assert_calls(2);
}

#[test]
fn test_ratelimit_remaining_below_threshold_is_err() {
    let server = MockServer::start();
    let body_str = r#"
    {
        "id": 4,
        "default_branch": "main",
    }"#;

    let server_mock = server.mock({
        |when, then| {
            when.method(GET).path("/repos/jordilin/mr");
            then.status(200)
                .header("content-type", "application/json")
                // below threshold
                .header("x-ratelimit-remaining", "5")
                .body(body_str);
        }
    });

    let url = format!("http://{}/repos/jordilin/mr", server.address());

    let runner = Client::new(NoCache, Arc::new(ConfigMock::new()), false);
    let mut request = Request::<()>::new(&url, Method::GET);

    match runner.run(&mut request) {
        Ok(_) => panic!("Expected error"),
        Err(err) => match err.downcast_ref::<GRError>() {
            Some(GRError::RateLimitExceeded(_)) => (),
            _ => panic!("Expected RateLimitExceeded error"),
        },
    }
    server_mock.assert_calls(1);
}

#[test]
fn test_ratelimit_remaining_above_threshold_is_ok() {
    let server = MockServer::start();
    let body_str = r#"
    {
        "id": 4,
        "default_branch": "main",
    }"#;

    let server_mock = server.mock({
        |when, then| {
            when.method(GET).path("/repos/jordilin/mr");
            then.status(200)
                .header("content-type", "application/json")
                // above threshold
                .header("RateLimit-Remaining", "15")
                .body(body_str);
        }
    });

    let url = format!("http://{}/repos/jordilin/mr", server.address());

    let runner = Client::new(NoCache, Arc::new(ConfigMock::new()), false);
    let mut request = Request::<()>::new(&url, Method::GET);

    assert!(runner.run(&mut request).is_ok());
    server_mock.assert_calls(1);
}
