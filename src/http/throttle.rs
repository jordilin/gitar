use std::thread;

use rand::Rng;

use crate::{io::FlowControlHeaders, log_info, time::Milliseconds};

/// Throttle strategy
pub trait ThrottleStrategy {
    /// Throttle the request based on optional flow control headers.
    /// Implementors might use the headers to adjust the throttling or ignore
    /// them altogether. Ex. strategies could be a fixed delay, random, or based
    /// on rate limiting headers.
    fn throttle(&self, response: Option<&FlowControlHeaders>);
}

pub struct Fixed {
    delay: Milliseconds,
}

impl Fixed {
    pub fn new(delay: Milliseconds) -> Self {
        Self { delay }
    }
}

impl ThrottleStrategy for Fixed {
    fn throttle(&self, _response: Option<&FlowControlHeaders>) {
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
    fn throttle(&self, _response: Option<&FlowControlHeaders>) {
        let mut rng = rand::thread_rng();
        let wait_time = rng.gen_range(*self.delay_min..=*self.delay_max);
        log_info!("Sleeping for {} milliseconds", wait_time);
        thread::sleep(std::time::Duration::from_millis(wait_time));
    }
}
