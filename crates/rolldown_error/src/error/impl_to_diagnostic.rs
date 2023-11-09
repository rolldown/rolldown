use crate::{BuildError, Diagnostic};

impl BuildError {
  pub fn to_diagnostic(&self) -> Diagnostic {
    let code = self.code();

    match self {
      Self::UnresolvedEntry(err) => {
        Diagnostic { code, summary: err.to_string(), ..Default::default() }
      }
      Self::ExternalEntry(err) => {
        Diagnostic { code, summary: err.to_string(), ..Default::default() }
      }
      Self::UnresolvedImport(err) => {
        Diagnostic { code, summary: err.to_string(), ..Default::default() }
      }
      Self::Io(err) => Diagnostic { code, summary: err.to_string(), ..Default::default() },
      Self::Napi { status, reason } => Diagnostic {
        code,
        summary: format!("Napi error: {status}: {reason}"),
        ..Default::default()
      },
    }
  }
}
