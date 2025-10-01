use napi_derive::napi;
use rolldown_error::BuildDiagnostic;

use crate::types::{binding_outputs::to_js_diagnostic, error::BindingError};

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

#[napi(discriminant = "type")]
#[derive(Debug)]
pub enum BindingHmrUpdate {
  Patch {
    code: String,
    filename: String,
    sourcemap: Option<String>,
    sourcemap_filename: Option<String>,
    hmr_boundaries: Vec<BindingHmrBoundaryOutput>,
  },
  FullReload {
    reason: Option<String>,
  },
  Noop,
}

#[napi]
#[derive(Debug)]
pub struct BindingClientHmrUpdate {
  client_id: String,
  update: BindingHmrUpdate,
}

#[napi]
impl BindingClientHmrUpdate {
  pub fn new(client_id: String, update: BindingHmrUpdate) -> Self {
    Self { client_id, update }
  }

  #[napi(getter)]
  pub fn client_id(&self) -> String {
    self.client_id.clone()
  }

  #[napi(getter)]
  pub fn update(&mut self) -> BindingHmrUpdate {
    std::mem::replace(&mut self.update, BindingHmrUpdate::Noop)
  }
}

impl From<rolldown_common::HmrUpdate> for BindingHmrUpdate {
  fn from(value: rolldown_common::HmrUpdate) -> Self {
    match value {
      rolldown_common::HmrUpdate::Patch(patch) => Self::Patch {
        code: patch.code,
        filename: patch.filename,
        sourcemap: patch.sourcemap,
        sourcemap_filename: patch.sourcemap_filename,
        hmr_boundaries: patch.hmr_boundaries.into_iter().map(Into::into).collect(),
      },
      rolldown_common::HmrUpdate::FullReload { reason } => {
        Self::FullReload { reason: Some(reason) }
      }
      rolldown_common::HmrUpdate::Noop => Self::Noop,
    }
  }
}

impl From<rolldown_common::ClientHmrUpdate> for BindingClientHmrUpdate {
  fn from(value: rolldown_common::ClientHmrUpdate) -> Self {
    Self { client_id: value.client_id, update: BindingHmrUpdate::from(value.update) }
  }
}

#[napi(discriminant = "type", object_from_js = false)]
pub enum BindingGenerateHmrPatchReturn {
  Ok(Vec<BindingHmrUpdate>),
  Error(Vec<BindingError>),
}

impl BindingGenerateHmrPatchReturn {
  pub fn from_errors(diagnostics: Vec<BuildDiagnostic>, cwd: std::path::PathBuf) -> Self {
    Self::Error(
      diagnostics.iter().map(|diagnostic| to_js_diagnostic(diagnostic, cwd.clone())).collect(),
    )
  }
}
