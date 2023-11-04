use crate::{BuildError, Diagnostic};

impl BuildError {
  pub fn to_diagnostic(&self) -> Diagnostic {
    let code = self.code();

    match self {
      BuildError::UnresolvedEntry(err) => {
        Diagnostic { code, summary: err.to_string(), ..Default::default() }
      }
      BuildError::ExternalEntry(err) => {
        Diagnostic { code, summary: err.to_string(), ..Default::default() }
      }
      BuildError::UnresolvedImport(err) => {
        Diagnostic { code, summary: err.to_string(), ..Default::default() }
      }
      BuildError::Napi { status, reason } => Diagnostic {
        code,
        summary: format!("Napi error: {}: {}", status, reason),
        ..Default::default()
      },
    }
  }
}
