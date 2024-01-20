use std::collections::HashMap;

use gr::cache::{Cache, InMemoryCache, NoCache};
use gr::config::ConfigProperties;
use gr::http::{Client, Method, Request};
use gr::io::{HttpRunner, Response, ResponseField};
use httpmock::prelude::*;
use httpmock::Method::{GET, PATCH, POST};

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

    let runner = Client::new(NoCache, ConfigMock::new(), false);
    let mut request = Request::<()>::new(&server.url("/repos/jordilin/mr"), Method::GET);
    let response = runner.run(&mut request).unwrap();
    assert_eq!(response.status, 200);
    assert!(response.body.contains("id"));
    server_mock.assert();
}

#[test]
fn test_http_runner_server_down() {
    let runner = Client::new(NoCache, ConfigMock::new(), false);
    let mut request = Request::<()>::new("http://localhost:8091/repos/jordilin/mr", Method::GET);
    let err = runner.run(&mut request).unwrap_err();
    assert!(err.to_string().contains("Connection refused"));
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

    let runner = Client::new(NoCache, ConfigMock::new(), false);
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

    let response = Response::new()
        .with_status(200)
        .with_body(body_str.to_string());

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
    let runner = Client::new(cache, ConfigMock::new(), false);
    let mut request = Request::<()>::new(&url, Method::GET);

    // Execute the client
    let response = runner.run(&mut request).unwrap();
    assert_eq!(response.status, 200);
    assert!(response.body.contains("id"));

    // Mock was never called. We used the cache
    server_mock.assert_hits(0);
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
                .body("");
        }
    });

    let body_str = r#"
    {
        "id": 4,
        "default_branch": "main",
    }"#;

    let mut headers = HashMap::new();
    headers.insert("etag".to_string(), "1234".to_string());
    headers.insert("Max-Age".to_string(), "0".to_string());
    let response = Response::new()
        .with_status(200)
        .with_body(body_str.to_string())
        .with_headers(headers);
    let url = format!("http://{}/repos/jordilin/mr/members", server.address());
    let mut cache = InMemoryCache::default();
    cache.set(&url, &response).unwrap();
    cache.expire();

    let runner = Client::new(&cache, ConfigMock::new(), false);
    let mut request = Request::<()>::new(&url, Method::GET);

    let response = runner.run(&mut request).unwrap();
    assert_eq!(response.status, 200);
    assert!(response.body.contains("id"));

    // While do we have a cache, the cache was expired, hence we expect the
    // server to be hit with a 304 status and a If-None-Match header set in the
    // request.
    server_mock.assert_hits(1);
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
    let runner = Client::new(cache, ConfigMock::new(), false);
    let mut request = Request::<()>::new(&url, Method::GET);

    // Execute the client
    let response = runner.run(&mut request).unwrap();
    assert_eq!(response.status, 200);
    assert!(response.body.contains("id"));

    // Verify the endpoint was hit
    server_mock.assert_hits(1);

    // Call is cached now
    // Do a second request and verify that the mock was not called
    let mut request = Request::<()>::new(&url, Method::GET);

    // Execute the client
    let response = runner.run(&mut request).unwrap();
    assert_eq!(response.status, 200);
    assert!(response.body.contains("id"));

    // Number of calls received is still 1 from the previous call
    server_mock.assert_hits(1);
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
    let runner = Client::new(cache, ConfigMock::new(), false);
    let mut request = Request::<()>::new(&url, Method::POST);

    // Execute the client
    let response = runner.run(&mut request).unwrap();
    assert_eq!(response.status, 200);
    assert!(response.body.contains("id"));

    // Verify the endpoint was hit
    server_mock.assert_hits(1);

    // Call is cached now
    // Do a second request and verify that the mock was not called
    let mut request = Request::<()>::new(&url, Method::POST);

    // Execute the client
    let response = runner.run(&mut request).unwrap();
    assert_eq!(response.status, 200);
    assert!(response.body.contains("id"));

    // Number of calls received is still 1 from the previous call
    server_mock.assert_hits(2);
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

    let runner = Client::new(NoCache, ConfigMock::new(), false);
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
    let runner = Client::new(cache, ConfigMock::new(), true);
    let mut request = Request::<()>::new(&url, Method::GET);

    // Execute the client
    let response = runner.run(&mut request).unwrap();
    assert_eq!(response.status, 200);
    assert!(response.body.contains("id"));

    // Verify the endpoint was hit
    server_mock.assert_hits(1);

    // Call is cached now
    // Do a second request and verify that the mock was not called
    let mut request = Request::<()>::new(&url, Method::GET);

    // Execute the client
    let response = runner.run(&mut request).unwrap();
    assert_eq!(response.status, 200);
    assert!(response.body.contains("id"));

    // We enforce cache refreshment, so the number of calls received by the HTTP
    // server is 2.
    server_mock.assert_hits(2);
}
