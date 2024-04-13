#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => (
        {
            info!($($arg)*);
        }
    );
}

#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => (
        {
            debug!($($arg)*);
        }
    );
}

#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => (
        {
            error!($($arg)*);
        }
    );
}
