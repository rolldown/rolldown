use napi::Env;
use napi_derive::napi;
use rolldown_error::BuildDiagnostic;
use rolldown_utils::unique_arc::WeakRef;

use crate::types::binding_outputs::to_js_diagnostic;

#[allow(clippy::rc_buffer)]
#[napi]
pub struct BindingHookError {
  errors: WeakRef<Vec<BuildDiagnostic>>,
  cwd: std::path::PathBuf,
}

#[napi]
impl BindingHookError {
  pub fn new(errors: WeakRef<Vec<BuildDiagnostic>>, cwd: std::path::PathBuf) -> Self {
    Self { errors, cwd }
  }

  #[napi(getter)]
<<<<<<< HEAD
  pub fn errors(&self, env: Env) -> napi::Result<Vec<napi::JsUnknown>> {
    self.errors.with_inner(|errors| {
      errors.iter().map(|diagnostic| to_js_diagnostic(diagnostic, self.cwd.clone(), env)).collect()
    })
=======
  pub fn errors(&self, env: Env) -> napi::Result<Vec<napi::Either<napi::JsError, napi::JsObject>>> {
    self
      .errors
      .iter()
      .map(|diagnostic| to_js_diagnostic(diagnostic, self.cwd.clone(), env))
      .collect()
>>>>>>> 504ea76c0 (fix: avoid one JsError into_unknown twice (#3020))
  }
}
