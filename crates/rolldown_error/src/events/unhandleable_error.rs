use crate::{DiagnosticOptions, EventKind};

use super::BuildEvent;

/// This is used for returning errors that are not expected to be handled by rolldown. Such as
/// - Error of converting u64 to usize in a platform that usize is 32-bit.
/// - ...
///   Handling such errors is meaningless.
///
/// Notice:
/// - We might mark some errors as unhandleable for faster development, but we should convert them
///   to `BuildDiagnostic` to provide better error messages to users.
#[derive(Debug)]
pub struct UnhandleableError(anyhow::Error);

impl BuildEvent for UnhandleableError {
  fn kind(&self) -> EventKind {
    EventKind::UnhandleableError
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    format!("Something wrong inside the rolldown, please report this at https://github.com/rolldown/rolldown/issues.\n{}", self.0)
  }
}
