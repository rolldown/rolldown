use std::path::PathBuf;

use oxc_resolver::ResolveError;

use crate::{
  build_diagnostic::diagnostic::Diagnostic, types::diagnostic_options::DiagnosticOptions,
  utils::resolve_error_to_message,
};

use super::BuildEvent;

#[derive(Debug)]
pub struct TsConfigError {
  pub reason: ResolveError,
}

impl BuildEvent for TsConfigError {
  fn kind(&self) -> crate::types::event_kind::EventKind {
    crate::types::event_kind::EventKind::TsConfigError
  }

  fn message(&self, opts: &DiagnosticOptions) -> String {
    let tsconfig_path = match &self.reason {
      ResolveError::TsconfigNotFound(path)
      | ResolveError::TsconfigSelfReference(path)
      | ResolveError::TsconfigLoadFailed { path, .. } => Some(path.as_path()),
      ResolveError::TsconfigCircularExtend(paths) => paths.paths().first().map(PathBuf::as_path),
      _ => None,
    };
    let reason = match &self.reason {
      ResolveError::TsconfigLoadFailed { source, .. } => match source.as_ref() {
        ResolveError::Json(json_error) => json_error.message.clone(),
        source => resolve_error_to_message(source, opts),
      },
      reason => resolve_error_to_message(reason, opts),
    };
    let tsconfig =
      tsconfig_path.map_or_else(String::new, |path| format!(" '{}'", opts.stabilize_path(path)));
    format!("Failed to load tsconfig{tsconfig}: {reason}")
  }

  fn on_diagnostic(&self, diagnostic: &mut Diagnostic, opts: &DiagnosticOptions) {
    diagnostic.title = self.message(opts);
  }
}
