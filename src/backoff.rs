use std::sync::Arc;

use serde::Serialize;

use crate::error::{AddContext, GRError};
use crate::io::{HttpRunner, RateLimitHeader};
use crate::{error, log_info, Result};
use crate::{http::Request, io::Response, time::Seconds};
use crate::{log_error, Error};

/// ExponentialBackoff wraps an HttpRunner and retries requests with an
/// exponential backoff retry mechanism.
pub struct ExponentialBackoff<'a, R> {
    runner: &'a Arc<R>,
    max_retries: u32,
    num_retries: u32,
    rate_limit_header: RateLimitHeader,
    now: fn() -> Seconds,
}

impl<'a, R> ExponentialBackoff<'a, R> {
    pub fn new(runner: &'a Arc<R>, max_retries: u32, now: fn() -> Seconds) -> Self {
        ExponentialBackoff {
            runner,
            max_retries,
            num_retries: 0,
            rate_limit_header: RateLimitHeader::default(),
            now,
        }
    }

    fn wait_time(&mut self) -> Seconds {
        // https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api?apiVersion=2022-11-28#exceeding-the-rate-limit
        let now = (self.now)();
        let mut base_wait_time = if self.rate_limit_header.reset > now {
            self.rate_limit_header.reset - now
        } else {
            // default to 1 minute.
            Seconds::new(60)
        };
        if self.rate_limit_header.retry_after > Seconds::new(0) {
            base_wait_time = self.rate_limit_header.retry_after;
        }
        let wait_time = base_wait_time + 2u64.pow(self.num_retries).into();
        log_info!("Waiting for {} seconds", wait_time);
        wait_time
    }

    /// Checks if the error is a candidate for retrying the request. A request
    /// can be retried if we are being rate limited or if there is a network
    /// outage.
    fn should_retry_on_error(&self, err: &Error) -> Option<RateLimitHeader> {
        return match err.downcast_ref::<error::GRError>() {
            Some(error::GRError::RateLimitExceeded(headers)) => Some(headers.clone()),
            Some(error::GRError::HttpTransportError(_)) => Some(RateLimitHeader::default()),
            _ => None,
        };
    }
}

impl<'a, R: HttpRunner<Response = Response>> ExponentialBackoff<'a, R> {
    pub fn retry_on_error<T: Serialize>(&mut self, request: &mut Request<T>) -> Result<Response> {
        loop {
            match self.runner.run(request) {
                Ok(response) => return Ok(response),
                Err(err) => {
                    if self.max_retries == 0 {
                        return Err(err);
                    }
                    log_error!("Error: {}", err);
                    log_info!(
                        "Backoff enabled re-trying {} out of {}",
                        self.num_retries + 1,
                        self.max_retries
                    );
                    if let Some(headers) = self.should_retry_on_error(&err) {
                        self.rate_limit_header = headers;
                        self.num_retries += 1;
                        if self.num_retries <= self.max_retries {
                            self.runner.throttle(self.wait_time().into());
                            continue;
                        }
                        return Err(GRError::ExponentialBackoffMaxRetriesReached(format!(
                            "Retried the request {} times",
                            self.max_retries
                        )))
                        .err_context(err);
                    } else {
                        return Err(err);
                    }
                }
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        http::{self, Headers, Resource},
        test::utils::MockRunner,
        time::Milliseconds,
    };

    use super::*;

    fn ratelimited_with_headers(remaining: u32, reset: u64, retry_after: u64) -> Response {
        let mut headers = Headers::new();
        headers.set("x-ratelimit-remaining".to_string(), remaining.to_string());
        headers.set("x-ratelimit-reset".to_string(), reset.to_string());
        headers.set("retry-after".to_string(), retry_after.to_string());
        Response::builder()
            .status(429)
            .headers(headers)
            .build()
            .unwrap()
    }

    fn ratelimited_with_no_headers() -> Response {
        Response::builder().status(429).build().unwrap()
    }

    fn response_ok() -> Response {
        Response::builder().status(200).build().unwrap()
    }

    fn now_mock() -> Seconds {
        Seconds::new(1712814151)
    }

    #[test]
    fn test_exponential_backoff_retries_and_succeeds() {
        let reset = now_mock() + Seconds::new(60);
        let responses = vec![
            response_ok(),
            ratelimited_with_no_headers(),
            ratelimited_with_headers(10, *reset, 60),
        ];
        let client = Arc::new(MockRunner::new(responses));
        let mut request: Request<()> = Request::builder()
            .resource(Resource::new("http://localhost", None))
            .method(http::Method::GET)
            .build()
            .unwrap();
        let mut backoff = ExponentialBackoff::new(&client, 3, now_mock);
        backoff.retry_on_error(&mut request).unwrap();
        assert_eq!(2, *client.throttled());
    }

    #[test]
    fn test_exponential_backoff_retries_and_fails_after_max_retries_reached() {
        let reset = now_mock() + Seconds::new(60);
        let responses = vec![
            response_ok(),
            ratelimited_with_no_headers(),
            ratelimited_with_headers(10, *reset, 60),
        ];
        let client = Arc::new(MockRunner::new(responses));
        let mut request: Request<()> = Request::builder()
            .resource(Resource::new("http://localhost", None))
            .method(http::Method::GET)
            .build()
            .unwrap();
        let mut backoff = ExponentialBackoff::new(&client, 1, now_mock);
        match backoff.retry_on_error(&mut request) {
            Ok(_) => panic!("Expected max retries reached error"),
            Err(err) => match err.downcast_ref::<error::GRError>() {
                Some(error::GRError::ExponentialBackoffMaxRetriesReached(_)) => {
                    assert_eq!(1, *client.throttled());
                    assert_eq!(Milliseconds::new(62000), *client.milliseconds_throttled());
                }
                _ => panic!("Expected max retries reached error"),
            },
        }
    }

    #[test]
    fn test_if_max_retries_is_zero_tries_once() {
        let responses = vec![response_ok()];
        let client = Arc::new(MockRunner::new(responses));
        let mut request: Request<()> = Request::builder()
            .resource(Resource::new("http://localhost", None))
            .method(http::Method::GET)
            .build()
            .unwrap();
        let mut backoff = ExponentialBackoff::new(&client, 0, now_mock);
        backoff.retry_on_error(&mut request).unwrap();
        assert_eq!(0, *client.throttled());
    }

    #[test]
    fn test_if_max_retries_is_zero_tries_once_and_fails() {
        let responses = vec![ratelimited_with_no_headers()];
        let client = Arc::new(MockRunner::new(responses));
        let mut request: Request<()> = Request::builder()
            .resource(Resource::new("http://localhost", None))
            .method(http::Method::GET)
            .build()
            .unwrap();
        let mut backoff = ExponentialBackoff::new(&client, 0, now_mock);
        match backoff.retry_on_error(&mut request) {
            Ok(_) => panic!("Expected rate limit exceeded error"),
            Err(err) => match err.downcast_ref::<error::GRError>() {
                Some(error::GRError::RateLimitExceeded(_)) => {}
                _ => panic!("Expected rate limit exceeded error"),
            },
        }
        assert_eq!(0, *client.throttled());
    }

    #[test]
    fn test_time_to_reset_is_zero() {
        let responses = vec![
            response_ok(),
            ratelimited_with_no_headers(),
            ratelimited_with_headers(10, 0, 0),
        ];
        let client = Arc::new(MockRunner::new(responses));
        let mut request: Request<()> = Request::builder()
            .resource(Resource::new("http://localhost", None))
            .method(http::Method::GET)
            .build()
            .unwrap();
        let mut backoff = ExponentialBackoff::new(&client, 3, now_mock);
        backoff.retry_on_error(&mut request).unwrap();
        assert_eq!(2, *client.throttled());
        // 60 secs base wait, 1st retry 2^1 = 2 => 62000 milliseconds
        // 60 secs base wait, 2nd retry 2^2 = 4 => 64000 milliseconds
        // Total wait 126000
        assert_eq!(Milliseconds::new(126000), *client.milliseconds_throttled());
    }

    #[test]
    fn test_retry_after_used_if_provided() {
        let reset = now_mock() + Seconds::new(120);
        let responses = vec![
            response_ok(),
            ratelimited_with_headers(10, 0, 65),
            ratelimited_with_headers(10, *reset, 61),
        ];
        let client = Arc::new(MockRunner::new(responses));
        let mut request: Request<()> = Request::builder()
            .resource(Resource::new("http://localhost", None))
            .method(http::Method::GET)
            .build()
            .unwrap();
        let mut backoff = ExponentialBackoff::new(&client, 3, now_mock);
        backoff.retry_on_error(&mut request).unwrap();
        assert_eq!(2, *client.throttled());
        // 61 secs base wait, 1st retry 2^1 = 2 => 63000 milliseconds
        // 65 secs base wait, 2nd retry 2^2 = 4 => 69000 milliseconds
        // Total wait 132000
        assert_eq!(Milliseconds::new(132000), *client.milliseconds_throttled());
    }

    #[test]
    fn test_reset_time_future_and_no_retry_after() {
        let reset_first = now_mock() + Seconds::new(120);
        let reset_second = now_mock() + Seconds::new(61);
        let responses = vec![
            response_ok(),
            ratelimited_with_headers(10, *reset_second, 0),
            ratelimited_with_headers(10, *reset_first, 0),
        ];
        let client = Arc::new(MockRunner::new(responses));
        let mut request: Request<()> = Request::builder()
            .resource(Resource::new("http://localhost", None))
            .method(http::Method::GET)
            .build()
            .unwrap();
        let mut backoff = ExponentialBackoff::new(&client, 3, now_mock);
        backoff.retry_on_error(&mut request).unwrap();
        assert_eq!(2, *client.throttled());
        // 120 secs base wait, 1st retry 2^1 = 2 => 122000 milliseconds
        // 61 secs base wait, 2nd retry 2^2 = 4 => 65000 milliseconds
        // Total wait 187000
        assert_eq!(Milliseconds::new(187000), *client.milliseconds_throttled());
    }
}
