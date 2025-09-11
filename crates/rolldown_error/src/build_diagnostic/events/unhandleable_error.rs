use std::{borrow::Cow, fmt::Display};

use crate::{types::diagnostic_options::DiagnosticOptions, types::event_kind::EventKind};

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
pub struct UnhandleableError(pub(crate) anyhow::Error);

impl BuildEvent for UnhandleableError {
  fn kind(&self) -> EventKind {
    EventKind::UnhandleableError
  }

  fn message(&self, _opts: &DiagnosticOptions) -> String {
    if let Some(inner_err) = self.0.downcast_ref::<CausedPlugin>() {
      let source = self.0.source().expect("Error with CausedPlugin should have a source");
      return format!(
        "Something went wrong inside native plugin `{}`. Please report this problem at https://github.com/rolldown/rolldown/issues.\n{}",
        inner_err.plugin, source
      );
    }

    format!(
      "Something went wrong inside rolldown, please report this problem at https://github.com/rolldown/rolldown/issues.\n{}",
      self.0
    )
  }
}

#[derive(Debug)]
pub struct CausedPlugin {
  pub plugin: Cow<'static, str>,
}

impl CausedPlugin {
  pub fn new(plugin: Cow<'static, str>) -> Self {
    Self { plugin }
  }
}

impl Display for CausedPlugin {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "caused by plugin `{}`", self.plugin)
  }
}
