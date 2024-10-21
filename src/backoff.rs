use std::sync::Arc;

use serde::Serialize;

use crate::error::{AddContext, GRError};
use crate::http::throttle::ThrottleStrategy;
use crate::io::{HttpRunner, RateLimitHeader};
use crate::log_error;
use crate::{error, log_info, Result};
use crate::{http::Request, io::HttpResponse, time::Seconds};

/// ExponentialBackoff wraps an HttpRunner and retries requests with an
/// exponential backoff retry mechanism.
pub struct Backoff<'a, R> {
    runner: &'a Arc<R>,
    max_retries: u32,
    num_retries: u32,
    rate_limit_header: RateLimitHeader,
    default_delay_wait: Seconds,
    now: fn() -> Seconds,
    backoff_strategy: Box<dyn BackOffStrategy>,
    throttler: Box<dyn ThrottleStrategy>,
}

impl<'a, R> Backoff<'a, R> {
    pub fn new(
        runner: &'a Arc<R>,
        max_retries: u32,
        default_delay_wait: u64,
        now: fn() -> Seconds,
        strategy: Box<dyn BackOffStrategy>,
        throttler_strategy: Box<dyn ThrottleStrategy>,
    ) -> Self {
        Backoff {
            runner,
            max_retries,
            num_retries: 0,
            rate_limit_header: RateLimitHeader::default(),
            default_delay_wait: Seconds::new(default_delay_wait),
            now,
            backoff_strategy: strategy,
            throttler: throttler_strategy,
        }
    }

    fn log_backoff_enabled(&self) {
        log_info!("Backoff enabled with {} max retries", self.max_retries);
    }
}

impl<'a, R: HttpRunner<Response = HttpResponse>> Backoff<'a, R> {
    pub fn retry_on_error<T: Serialize>(
        &mut self,
        request: &mut Request<T>,
    ) -> Result<HttpResponse> {
        loop {
            if self.num_retries > 0 {
                log_info!(
                    "Retrying request {} out of {}",
                    self.num_retries,
                    self.max_retries
                );
            }
            match self.runner.run(request) {
                Ok(response) => return Ok(response),
                Err(err) => {
                    if self.max_retries == 0 {
                        return Err(err);
                    }
                    log_error!("Error: {}", err);
                    // https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api?apiVersion=2022-11-28#exceeding-the-rate-limit
                    match err.downcast_ref::<error::GRError>() {
                        Some(error::GRError::RateLimitExceeded(headers)) => {
                            self.rate_limit_header = *headers;
                            self.num_retries += 1;
                            if self.num_retries <= self.max_retries {
                                let now = (self.now)();
                                let mut base_wait_time = if self.rate_limit_header.reset > now {
                                    self.rate_limit_header.reset - now
                                } else {
                                    self.default_delay_wait
                                };
                                if self.rate_limit_header.retry_after > Seconds::new(0) {
                                    base_wait_time = self.rate_limit_header.retry_after;
                                }
                                self.log_backoff_enabled();
                                self.throttler.throttle_for(
                                    self.backoff_strategy
                                        .wait_time(base_wait_time, self.num_retries)
                                        .into(),
                                );
                                continue;
                            }
                        }
                        Some(
                            error::GRError::HttpTransportError(_)
                            | error::GRError::RemoteServerError(_),
                        ) => {
                            self.num_retries += 1;
                            if self.num_retries <= self.max_retries {
                                self.log_backoff_enabled();
                                self.throttler.throttle_for(
                                    self.backoff_strategy
                                        .wait_time(self.default_delay_wait, self.num_retries)
                                        .into(),
                                );
                                continue;
                            }
                        }
                        _ => {
                            return Err(err);
                        }
                    }
                    return Err(GRError::ExponentialBackoffMaxRetriesReached(format!(
                        "Retried the request {} times",
                        self.max_retries
                    )))
                    .err_context(err);
                }
            };
        }
    }
}

pub trait BackOffStrategy {
    fn wait_time(&self, base_wait: Seconds, num_retries: u32) -> Seconds;
}

pub struct Exponential;

impl BackOffStrategy for Exponential {
    fn wait_time(&self, base_wait: Seconds, num_retries: u32) -> Seconds {
        log_info!("Exponential backoff strategy enabled");
        let wait_time = base_wait + 2u64.pow(num_retries).into();
        log_info!("Waiting for {} seconds", wait_time);
        wait_time
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use crate::{
        http::{self, Headers, Resource},
        io::FlowControlHeaders,
        test::utils::{MockRunner, MockThrottler},
        time::Milliseconds,
    };

    use super::*;

    fn ratelimited_with_headers(remaining: u32, reset: u64, retry_after: u64) -> HttpResponse {
        let mut headers = Headers::new();
        headers.set("x-ratelimit-remaining".to_string(), remaining.to_string());
        headers.set("x-ratelimit-reset".to_string(), reset.to_string());
        headers.set("retry-after".to_string(), retry_after.to_string());
        let rate_limit_header =
            RateLimitHeader::new(remaining, Seconds::new(reset), Seconds::new(retry_after));
        let flow_control_headers =
            FlowControlHeaders::new(Rc::new(None), Rc::new(Some(rate_limit_header)));
        HttpResponse::builder()
            .status(429)
            .headers(headers)
            .flow_control_headers(flow_control_headers)
            .build()
            .unwrap()
    }

    fn ratelimited_with_no_headers() -> HttpResponse {
        HttpResponse::builder().status(429).build().unwrap()
    }

    fn response_ok() -> HttpResponse {
        HttpResponse::builder().status(200).build().unwrap()
    }

    fn response_server_error() -> HttpResponse {
        HttpResponse::builder().status(500).build().unwrap()
    }

    fn response_transport_error() -> HttpResponse {
        // Could be a timeout, connection error, etc. Status code
        // For testing purposes, the status code of -1 simulates a transport
        // error from the mock http runner.
        // TODO: Should move to enums instead at some point.
        HttpResponse::builder().status(-1).build().unwrap()
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
        let strategy = Box::new(Exponential);
        let throttler = Rc::new(MockThrottler::new(None));
        let bthrottler: Box<dyn ThrottleStrategy> = Box::new(Rc::clone(&throttler));
        let mut backoff = Backoff::new(&client, 3, 60, now_mock, strategy, bthrottler);
        backoff.retry_on_error(&mut request).unwrap();
        assert_eq!(2, *throttler.throttled());
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
        let strategy = Box::new(Exponential);
        let throttler = Rc::new(MockThrottler::new(None));
        let bthrottler: Box<dyn ThrottleStrategy> = Box::new(Rc::clone(&throttler));
        let mut backoff = Backoff::new(&client, 1, 60, now_mock, strategy, bthrottler);
        match backoff.retry_on_error(&mut request) {
            Ok(_) => panic!("Expected max retries reached error"),
            Err(err) => match err.downcast_ref::<error::GRError>() {
                Some(error::GRError::ExponentialBackoffMaxRetriesReached(_)) => {
                    assert_eq!(1, *throttler.throttled());
                    assert_eq!(
                        Milliseconds::new(62000),
                        *throttler.milliseconds_throttled()
                    );
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
        let strategy = Box::new(Exponential);
        let throttler = Rc::new(MockThrottler::new(None));
        let bthrottler: Box<dyn ThrottleStrategy> = Box::new(Rc::clone(&throttler));
        let mut backoff = Backoff::new(&client, 0, 60, now_mock, strategy, bthrottler);
        backoff.retry_on_error(&mut request).unwrap();
        assert_eq!(0, *throttler.throttled());
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
        let strategy = Box::new(Exponential);
        let throttler = Rc::new(MockThrottler::new(None));
        let bthrottler: Box<dyn ThrottleStrategy> = Box::new(Rc::clone(&throttler));
        let mut backoff = Backoff::new(&client, 0, 60, now_mock, strategy, bthrottler);
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
        let strategy = Box::new(Exponential);
        let throttler = Rc::new(MockThrottler::new(None));
        let bthrottler: Box<dyn ThrottleStrategy> = Box::new(Rc::clone(&throttler));
        let mut backoff = Backoff::new(&client, 3, 60, now_mock, strategy, bthrottler);
        backoff.retry_on_error(&mut request).unwrap();
        assert_eq!(2, *throttler.throttled());
        // 60 secs base wait, 1st retry 2^1 = 2 => 62000 milliseconds
        // 60 secs base wait, 2nd retry 2^2 = 4 => 64000 milliseconds
        // Total wait 126000
        assert_eq!(
            Milliseconds::new(126000),
            *throttler.milliseconds_throttled()
        );
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
        let strategy = Box::new(Exponential);
        let throttler = Rc::new(MockThrottler::new(None));
        let bthrottler: Box<dyn ThrottleStrategy> = Box::new(Rc::clone(&throttler));
        let mut backoff = Backoff::new(&client, 3, 60, now_mock, strategy, bthrottler);
        backoff.retry_on_error(&mut request).unwrap();
        assert_eq!(2, *throttler.throttled());
        // 61 secs base wait, 1st retry 2^1 = 2 => 63000 milliseconds
        // 65 secs base wait, 2nd retry 2^2 = 4 => 69000 milliseconds
        // Total wait 132000
        assert_eq!(
            Milliseconds::new(132000),
            *throttler.milliseconds_throttled()
        );
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
        let strategy = Box::new(Exponential);
        let throttler = Rc::new(MockThrottler::new(None));
        let bthrottler: Box<dyn ThrottleStrategy> = Box::new(Rc::clone(&throttler));
        let mut backoff = Backoff::new(&client, 3, 60, now_mock, strategy, bthrottler);
        backoff.retry_on_error(&mut request).unwrap();
        assert_eq!(2, *throttler.throttled());
        // 120 secs base wait, 1st retry 2^1 = 2 => 122000 milliseconds
        // 61 secs base wait, 2nd retry 2^2 = 4 => 65000 milliseconds
        // Total wait 187000
        assert_eq!(
            Milliseconds::new(187000),
            *throttler.milliseconds_throttled()
        );
    }

    #[test]
    fn test_retries_on_server_500_error() {
        let responses = vec![response_ok(), response_server_error()];
        let client = Arc::new(MockRunner::new(responses));
        let mut request: Request<()> = Request::builder()
            .resource(Resource::new("http://localhost", None))
            .method(http::Method::GET)
            .build()
            .unwrap();
        let strategy = Box::new(Exponential);
        let throttler = Rc::new(MockThrottler::new(None));
        let bthrottler: Box<dyn ThrottleStrategy> = Box::new(Rc::clone(&throttler));
        let mut backoff = Backoff::new(&client, 1, 60, now_mock, strategy, bthrottler);
        backoff.retry_on_error(&mut request).unwrap();
        assert_eq!(1, *throttler.throttled());
        // Success on 2nd retry. Wait time of 1min + 2^1 = 2 => 62000 milliseconds
        assert_eq!(
            Milliseconds::new(62000),
            *throttler.milliseconds_throttled()
        );
    }

    #[test]
    fn test_retries_on_server_500_error_and_fails_after_max_retries_reached() {
        let responses = vec![response_server_error(), response_server_error()];
        let client = Arc::new(MockRunner::new(responses));
        let mut request: Request<()> = Request::builder()
            .resource(Resource::new("http://localhost", None))
            .method(http::Method::GET)
            .build()
            .unwrap();
        let strategy = Box::new(Exponential);
        let throttler = Rc::new(MockThrottler::new(None));
        let bthrottler: Box<dyn ThrottleStrategy> = Box::new(Rc::clone(&throttler));
        let mut backoff = Backoff::new(&client, 1, 60, now_mock, strategy, bthrottler);
        match backoff.retry_on_error(&mut request) {
            Ok(_) => panic!("Expected max retries reached error"),
            Err(err) => match err.downcast_ref::<error::GRError>() {
                Some(error::GRError::ExponentialBackoffMaxRetriesReached(_)) => {
                    assert_eq!(1, *throttler.throttled());
                    assert_eq!(
                        Milliseconds::new(62000),
                        *throttler.milliseconds_throttled()
                    );
                }
                _ => panic!("Expected max retries reached error"),
            },
        }
    }

    #[test]
    fn test_retries_on_transport_error() {
        let responses = vec![response_ok(), response_transport_error()];
        let client = Arc::new(MockRunner::new(responses));
        let mut request: Request<()> = Request::builder()
            .resource(Resource::new("http://localhost", None))
            .method(http::Method::GET)
            .build()
            .unwrap();
        let strategy = Box::new(Exponential);
        let throttler = Rc::new(MockThrottler::new(None));
        let bthrottler: Box<dyn ThrottleStrategy> = Box::new(Rc::clone(&throttler));
        let mut backoff = Backoff::new(&client, 1, 60, now_mock, strategy, bthrottler);
        backoff.retry_on_error(&mut request).unwrap();
        assert_eq!(1, *throttler.throttled());
        // Success on 2nd retry. Wait time of 1min + 2^1 = 2 => 62000 milliseconds
        assert_eq!(
            Milliseconds::new(62000),
            *throttler.milliseconds_throttled()
        );
    }

    #[test]
    fn test_retries_on_transport_error_and_fails_after_max_retries_reached() {
        let responses = vec![response_transport_error(), response_transport_error()];
        let client = Arc::new(MockRunner::new(responses));
        let mut request: Request<()> = Request::builder()
            .resource(Resource::new("http://localhost", None))
            .method(http::Method::GET)
            .build()
            .unwrap();
        let strategy = Box::new(Exponential);
        let throttler = Rc::new(MockThrottler::new(None));
        let bthrottler: Box<dyn ThrottleStrategy> = Box::new(Rc::clone(&throttler));
        let mut backoff = Backoff::new(&client, 1, 60, now_mock, strategy, bthrottler);
        match backoff.retry_on_error(&mut request) {
            Ok(_) => panic!("Expected max retries reached error"),
            Err(err) => match err.downcast_ref::<error::GRError>() {
                Some(error::GRError::ExponentialBackoffMaxRetriesReached(_)) => {
                    assert_eq!(1, *throttler.throttled());
                    assert_eq!(
                        Milliseconds::new(62000),
                        *throttler.milliseconds_throttled()
                    );
                }
                _ => panic!("Expected max retries reached error"),
            },
        }
    }
}
