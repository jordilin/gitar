use std::fmt::Display;

use anyhow::{anyhow, Context, Result};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum GRError {
    #[error("Precondition not met error: {0}")]
    PreconditionNotMet(String),
    #[error("Time conversion error: {0}")]
    TimeConversionError(String),
    #[error("Configuration error: {0}")]
    ConfigurationError(String),
    #[error("Operation not supported for this resource: {0}")]
    OperationNotSupported(String),
    #[error("RateLimit exceeded: {0}")]
    RateLimitExceeded(String),
    #[error("Application error: {0}")]
    ApplicationError(String),
    // The remote server returned a JSON response that was not expected. The
    // contract was broken and would need new validation.
    #[error(
        "Remote unexpected response contract: Open issue at https://github.com/jordilin/gitar: {0}"
    )]
    RemoteUnexpectedResponseContract(String),
}

pub trait AddContext<T, E>: Context<T, E> {
    fn err_context<C: Display + Send + Sync + 'static>(self, msg: C) -> Result<T, anyhow::Error>
    where
        Self: Sized,
    {
        self.with_context(|| msg.to_string())
    }
}

impl<U, T, E> AddContext<T, E> for U where U: Context<T, E> {}

pub fn gen<T: AsRef<str>>(msg: T) -> anyhow::Error {
    anyhow!(msg.as_ref().to_string())
}
