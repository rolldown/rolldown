use napi_derive::napi;
use rolldown_error::BuildDiagnostic;

use crate::types::binding_outputs::{BindingError, to_js_diagnostic};

#[napi]
#[derive(Debug)]
pub struct BindingHmrOutput {
  patch: Option<BindingHmrOutputPatch>,
  errors: Option<rolldown_common::OutputsDiagnostics>,
}

#[napi]
impl BindingHmrOutput {
  #[napi(getter)]
  pub fn patch(&mut self) -> Option<BindingHmrOutputPatch> {
    self.patch.take()
  }

  #[napi(getter)]
  pub fn errors(&mut self) -> Vec<napi::Either<napi::JsError, BindingError>> {
    if let Some(rolldown_common::OutputsDiagnostics { diagnostics, cwd }) = self.errors.as_ref() {
      return diagnostics
        .iter()
        .map(|diagnostic| to_js_diagnostic(diagnostic, cwd.clone()))
        .collect();
    }
    vec![]
  }

  pub fn from_errors(diagnostics: Vec<BuildDiagnostic>, cwd: std::path::PathBuf) -> Self {
    let errors = rolldown_common::OutputsDiagnostics { diagnostics, cwd };
    Self { patch: None, errors: Some(errors) }
  }
}

impl From<rolldown_common::HmrOutput> for BindingHmrOutput {
  fn from(value: rolldown_common::HmrOutput) -> Self {
    Self { patch: Some(value.into()), errors: None }
  }
}

impl From<Option<rolldown_common::HmrOutput>> for BindingHmrOutput {
  fn from(value: Option<rolldown_common::HmrOutput>) -> Self {
    Self { patch: value.map(Into::into), errors: None }
  }
}

#[napi_derive::napi(object)]
#[derive(Debug)]
pub struct BindingHmrOutputPatch {
  pub code: String,
  pub filename: String,
  pub sourcemap: Option<String>,
  pub sourcemap_filename: Option<String>,
  pub hmr_boundaries: Vec<BindingHmrBoundaryOutput>,
  pub full_reload: bool,
  pub first_invalidated_by: Option<String>,
  pub is_self_accepting: bool,
  pub full_reload_reason: Option<String>,
}

impl From<rolldown_common::HmrOutput> for BindingHmrOutputPatch {
  fn from(value: rolldown_common::HmrOutput) -> Self {
    Self {
      code: value.code,
      filename: value.filename,
      sourcemap: value.sourcemap,
      sourcemap_filename: value.sourcemap_filename,
      hmr_boundaries: value.hmr_boundaries.into_iter().map(Into::into).collect(),
      full_reload: value.full_reload,
      first_invalidated_by: value.first_invalidated_by,
      is_self_accepting: value.is_self_accepting,
      full_reload_reason: value.full_reload_reason,
    }
  }
}

#[napi_derive::napi(object)]
#[derive(Debug)]
pub struct BindingHmrBoundaryOutput {
  pub boundary: String,
  pub accepted_via: String,
}

impl From<rolldown_common::HmrBoundaryOutput> for BindingHmrBoundaryOutput {
  fn from(value: rolldown_common::HmrBoundaryOutput) -> Self {
    Self { boundary: value.boundary.to_string(), accepted_via: value.accepted_via.to_string() }
  }
}
