#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => (
        {
            use crate::VERBOSE;
            let verbose = *VERBOSE.get().unwrap_or(&false);
            if verbose {
                info!($($arg)*);
            }
        }
    );
}

#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => (
        {
            use crate::VERBOSE;
            let verbose = *VERBOSE.get().unwrap_or(&false);
            if verbose {
                debug!($($arg)*);
            }
        }
    );
}
