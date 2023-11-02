mod error;
mod error_code;
mod utils;
pub use crate::error::BuildError;

pub type BuildResult<T> = Result<T, BuildError>;
