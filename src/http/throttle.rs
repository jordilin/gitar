//! Throttle module provides different strategies to throttle requests based on
//! flow control headers or provided delays.

use std::thread;

use rand::Rng;

use crate::{
    api_defaults::{DEFAULT_JITTER_MAX_MILLISECONDS, DEFAULT_JITTER_MIN_MILLISECONDS},
    io::FlowControlHeaders,
    log_debug, log_info,
    time::{self, Milliseconds, Seconds},
};

/// Throttle strategy
pub trait ThrottleStrategy {
    /// Throttle the request based on optional flow control headers.
    /// Implementers might use the headers to adjust the throttling or ignore
    /// them altogether. Ex. strategies could be a fixed delay, random, or based
    /// on rate limiting headers.
    fn throttle(&self, flow_control_headers: Option<&FlowControlHeaders>);
    /// Throttle for specific amount of time.
    fn throttle_for(&self, delay: Milliseconds) {
        log_info!("Throttling for : {} ms", delay);
        thread::sleep(std::time::Duration::from_millis(*delay));
    }
    /// Return strategy type
    fn strategy(&self) -> ThrottleStrategyType;
}

#[derive(Clone, Debug, PartialEq)]
pub enum ThrottleStrategyType {
    PreFixed,
    DynamicFixed,
    Random,
    AutoRate,
    NoThrottle,
}

/// Dynamically throttles for the amount of time specified in the throttle_for
/// method using the default trait implementation. As opposed to the PreFixed,
/// which takes a fixed delay in the constructor and throttles for that amount
/// of time every time.
pub struct DynamicFixed;

impl ThrottleStrategy for DynamicFixed {
    fn throttle(&self, _flow_control_headers: Option<&FlowControlHeaders>) {}
    fn strategy(&self) -> ThrottleStrategyType {
        ThrottleStrategyType::DynamicFixed
    }
}

pub struct PreFixed {
    delay: Milliseconds,
}

impl PreFixed {
    pub fn new(delay: Milliseconds) -> Self {
        Self { delay }
    }
}

impl ThrottleStrategy for PreFixed {
    fn throttle(&self, _flow_control_headers: Option<&FlowControlHeaders>) {
        log_info!("Throttling for: {} ms", self.delay);
        thread::sleep(std::time::Duration::from_millis(*self.delay));
    }
    fn strategy(&self) -> ThrottleStrategyType {
        ThrottleStrategyType::PreFixed
    }
}

pub struct Random {
    delay_min: Milliseconds,
    delay_max: Milliseconds,
}

impl Random {
    pub fn new(delay_min: Milliseconds, delay_max: Milliseconds) -> Self {
        Self {
            delay_min,
            delay_max,
        }
    }
}

impl ThrottleStrategy for Random {
    fn throttle(&self, _flow_control_headers: Option<&FlowControlHeaders>) {
        log_info!(
            "Throttling between: {} ms and {} ms",
            self.delay_min,
            self.delay_max
        );
        let mut rng = rand::thread_rng();
        let wait_time = rng.gen_range(*self.delay_min..=*self.delay_max);
        log_info!("Sleeping for {} milliseconds", wait_time);
        thread::sleep(std::time::Duration::from_millis(wait_time));
    }
    fn strategy(&self) -> ThrottleStrategyType {
        ThrottleStrategyType::Random
    }
}

#[derive(Default)]
pub struct NoThrottle;

impl NoThrottle {
    pub fn new() -> Self {
        Self {}
    }
}

impl ThrottleStrategy for NoThrottle {
    fn throttle(&self, _flow_control_headers: Option<&FlowControlHeaders>) {
        log_info!("No throttling enabled");
    }
    fn strategy(&self) -> ThrottleStrategyType {
        ThrottleStrategyType::NoThrottle
    }
}

/// AutoRate implements an automatic throttling algorithm that limits the
/// rate of requests based on flow control headers from the HTTP response plus a
/// fixed random delay to avoid being predictable and too fast for the server.
/// Inspiration ref: https://en.wikipedia.org/wiki/Leaky_bucket
pub struct AutoRate {
    /// Max interval milliseconds added to the automatic throttle. In order to
    /// avoid predictability, the minimum range is 1 second. The jitter is the
    /// max interval added to the automatic throttle. (1, jitter) milliseconds.
    jitter_max: Milliseconds,
    jitter_min: Milliseconds,
    now: fn() -> Seconds,
}

impl Default for AutoRate {
    fn default() -> Self {
        Self {
            jitter_max: Milliseconds::from(DEFAULT_JITTER_MAX_MILLISECONDS),
            jitter_min: Milliseconds::from(DEFAULT_JITTER_MIN_MILLISECONDS),
            now: time::now_epoch_seconds,
        }
    }
}

impl ThrottleStrategy for AutoRate {
    fn throttle(&self, flow_control_headers: Option<&FlowControlHeaders>) {
        if let Some(headers) = flow_control_headers {
            let rate_limit_headers = headers.get_rate_limit_header();
            match *rate_limit_headers {
                Some(headers) => {
                    // In order to avoid rate limited, we need to space the
                    // requests evenly using: time to ratelimit-reset
                    // (secs)/ratelimit-remaining (requests).
                    let now = *(self.now)();
                    log_debug!("Current epoch: {}", now);
                    log_debug!("Rate limit reset: {}", headers.reset);
                    let time_to_reset = headers.reset.saturating_sub(now);
                    log_debug!("Time to reset: {}", time_to_reset);
                    log_debug!("Remaining requests: {}", headers.remaining);
                    let delay = time_to_reset / headers.remaining as u64;
                    // Avoid predictability and being too fast. We could end up
                    // being too fast when the amount of remaining requests
                    // is high and the reset time is low. We additionally
                    // wait in between jitter_min and jitter_max milliseconds.
                    let additional_delay =
                        rand::thread_rng().gen_range(*self.jitter_min..=*self.jitter_max);
                    let total_delay = delay + additional_delay;
                    log_info!("AutoRate throttling enabled");
                    self.throttle_for(Milliseconds::from(total_delay));
                }
                None => {
                    // When the response has status 304 Not Modified, we don't get
                    // any rate limiting headers. In this case, we just throttle
                    // randomly between the min and max jitter.
                    let rand_delay_jitter =
                        rand::thread_rng().gen_range(*self.jitter_min..=*self.jitter_max);
                    log_info!("AutoRate throttling enabled");
                    self.throttle_for(Milliseconds::from(rand_delay_jitter));
                }
            }
        }
    }
    fn strategy(&self) -> ThrottleStrategyType {
        ThrottleStrategyType::AutoRate
    }
}
