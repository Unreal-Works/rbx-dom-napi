use std::any::Any;
use std::panic::{catch_unwind, AssertUnwindSafe};

use napi::{Error, Result, Status};

pub fn invalid_arg(message: impl Into<String>) -> Error {
    Error::new(Status::InvalidArg, message.into())
}

pub fn failure(message: impl Into<String>) -> Error {
    Error::new(Status::GenericFailure, message.into())
}

pub fn upstream_error(operation: &str, error: impl std::fmt::Display) -> Error {
    failure(format!("{operation} failed: {error}"))
}

pub fn catch_panic<T, F>(operation: &str, callback: F) -> Result<T>
where
    F: FnOnce() -> Result<T>,
{
    match catch_unwind(AssertUnwindSafe(callback)) {
        Ok(result) => result,
        Err(payload) => Err(failure(format!(
            "{operation} panicked: {}",
            panic_message(payload)
        ))),
    }
}

fn panic_message(payload: Box<dyn Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_owned()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "unknown panic payload".to_owned()
    }
}
