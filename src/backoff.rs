use std::sync::Arc;

use serde::Serialize;

use crate::error::{AddContext, GRError};
use crate::io::{HttpRunner, RateLimitHeader};
use crate::Error;
use crate::{error, log_info, Result};
use crate::{http::Request, io::Response, time::Seconds};

/// ExponentialBackoff wraps an HttpRunner and retries requests with an
/// exponential backoff retry mechanism.
pub struct ExponentialBackoff<'a, R> {
    runner: &'a Arc<R>,
    max_retries: u32,
    num_retries: u32,
    rate_limit_header: RateLimitHeader,
}

impl<'a, R> ExponentialBackoff<'a, R> {
    pub fn new(runner: &'a Arc<R>, max_retries: u32) -> Self {
        ExponentialBackoff {
            runner,
            max_retries,
            num_retries: 0,
            rate_limit_header: RateLimitHeader::default(),
        }
    }

    fn wait_time(&mut self) -> Seconds {
        let mut base_wait_time = self.rate_limit_header.reset;
        if self.rate_limit_header.retry_after > Seconds::new(0) {
            base_wait_time = self.rate_limit_header.retry_after;
        }
        if base_wait_time == Seconds::new(0) {
            // https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api?apiVersion=2022-11-28#exceeding-the-rate-limit
            // Wait for 60 seconds if no rate limit headers are present
            base_wait_time = Seconds::new(60);
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

    #[test]
    fn test_exponential_backoff_retries_and_succeeds() {
        let responses = vec![
            response_ok(),
            ratelimited_with_no_headers(),
            ratelimited_with_headers(10, 1658602270, 60),
        ];
        let client = Arc::new(MockRunner::new(responses));
        let mut request: Request<()> = Request::builder()
            .resource(Resource::new("http://localhost", None))
            .method(http::Method::GET)
            .build()
            .unwrap();
        let mut backoff = ExponentialBackoff::new(&client, 3);
        backoff.retry_on_error(&mut request).unwrap();
        assert_eq!(2, *client.throttled());
    }

    #[test]
    fn test_exponential_backoff_retries_and_fails_after_max_retries_reached() {
        let responses = vec![
            response_ok(),
            ratelimited_with_no_headers(),
            ratelimited_with_headers(10, 1658602270, 60),
        ];
        let client = Arc::new(MockRunner::new(responses));
        let mut request: Request<()> = Request::builder()
            .resource(Resource::new("http://localhost", None))
            .method(http::Method::GET)
            .build()
            .unwrap();
        let mut backoff = ExponentialBackoff::new(&client, 1);
        match backoff.retry_on_error(&mut request) {
            Ok(_) => panic!("Expected max retries reached error"),
            Err(err) => match err.downcast_ref::<error::GRError>() {
                Some(error::GRError::ExponentialBackoffMaxRetriesReached(_)) => {}
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
        let mut backoff = ExponentialBackoff::new(&client, 0);
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
        let mut backoff = ExponentialBackoff::new(&client, 0);
        match backoff.retry_on_error(&mut request) {
            Ok(_) => panic!("Expected rate limit exceeded error"),
            Err(err) => match err.downcast_ref::<error::GRError>() {
                Some(error::GRError::RateLimitExceeded(_)) => {}
                _ => panic!("Expected rate limit exceeded error"),
            },
        }
        assert_eq!(0, *client.throttled());
    }
}
