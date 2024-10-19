use std::thread;

use rand::Rng;

use crate::{io::FlowControlHeaders, log_info, time::Milliseconds};

/// Throttle strategy
pub trait ThrottleStrategy {
    /// Throttle the request based on optional flow control headers.
    /// Implementors might use the headers to adjust the throttling or ignore
    /// them altogether. Ex. strategies could be a fixed delay, random, or based
    /// on rate limiting headers.
    fn throttle(&self, flow_control_headers: Option<&FlowControlHeaders>);
    /// Throttle for specific amount of time.
    fn throttle_for(&self, delay: Milliseconds) {
        log_info!("Throttling for backoff: {} ms", delay);
        thread::sleep(std::time::Duration::from_millis(*delay));
    }
}

/// Dynamically throttles for the amount of time specified in the throttle_for
/// method using the default trait implementation. As opposed to the PreFixed,
/// which takes a fixed delay in the constructor and throttles for that amount
/// of time every time.
pub struct DynamicFixed;

impl ThrottleStrategy for DynamicFixed {
    fn throttle(&self, _flow_control_headers: Option<&FlowControlHeaders>) {}
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
}
