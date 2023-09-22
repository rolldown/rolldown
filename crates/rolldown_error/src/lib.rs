mod error;
mod error_kind;
mod errors;
mod utils;
pub use crate::{error::Error, errors::Errors};

pub type Result<T> = std::result::Result<T, Error>;
