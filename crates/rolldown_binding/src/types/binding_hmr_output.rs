use napi_derive::napi;
use rolldown_error::BuildDiagnostic;

use super::{
  binding_hmr_update::BindingHmrUpdate, binding_outputs::to_binding_error, error::BindingError,
};

#[napi]
#[derive(Debug)]
pub struct BindingHmrOutput {
  patch: Option<BindingHmrUpdate>,
  errors: Option<rolldown_common::OutputsDiagnostics>,
}

#[napi]
impl BindingHmrOutput {
  pub fn new(
    patch: Option<BindingHmrUpdate>,
    errors: Option<rolldown_common::OutputsDiagnostics>,
  ) -> Self {
    Self { patch, errors }
  }

  #[napi(getter)]
  pub fn patch(&mut self) -> Option<BindingHmrUpdate> {
    self.patch.take()
  }

  #[napi(getter)]
  pub fn errors(&mut self) -> Vec<BindingError> {
    if let Some(rolldown_common::OutputsDiagnostics { diagnostics, cwd }) = self.errors.as_ref() {
      return diagnostics
        .iter()
        .map(|diagnostic| to_binding_error(diagnostic, cwd.clone()))
        .collect();
    }
    vec![]
  }

  pub fn from_errors(diagnostics: Vec<BuildDiagnostic>, cwd: std::path::PathBuf) -> Self {
    let errors = rolldown_common::OutputsDiagnostics { diagnostics, cwd };
    Self { patch: None, errors: Some(errors) }
  }
}
