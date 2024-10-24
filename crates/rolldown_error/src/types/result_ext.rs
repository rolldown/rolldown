use crate::{type_aliases::UnaryBuildResult, BuildDiagnostic};
use std::error::Error as StdError;

pub trait ResultExt<Val> {
  /// This method is used to make converting outside errors to `BuildDiagnostic` easier.
  /// For example, handling errors like converting u64 to usize in a platform that usize is 32-bit is meaningless for
  /// rolldown. So we just convert them to `BuildDiagnostic::unhandleable_error` to provide better dx around errors.
  fn map_error_to_unhandleable(self) -> UnaryBuildResult<Val>;
}

impl<Val, Err> ResultExt<Val> for Result<Val, Err>
where
  Err: StdError + Send + Sync + 'static,
{
  fn map_error_to_unhandleable(self) -> UnaryBuildResult<Val> {
    self.map_err(|err| {
      let err = anyhow::Error::new(err);
      BuildDiagnostic::unhandleable_error(err)
    })
  }
}
