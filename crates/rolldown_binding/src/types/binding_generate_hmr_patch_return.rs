use napi_derive::napi;
use rolldown_error::BuildDiagnostic;

use super::{
  binding_hmr_update::BindingHmrUpdate, binding_outputs::to_binding_error, error::BindingError,
};

#[napi(discriminant = "type", object_from_js = false)]
pub enum BindingGenerateHmrPatchReturn {
  Ok(Vec<BindingHmrUpdate>),
  Error(Vec<BindingError>),
}

impl BindingGenerateHmrPatchReturn {
  pub fn from_errors(diagnostics: Vec<BuildDiagnostic>, cwd: std::path::PathBuf) -> Self {
    Self::Error(
      diagnostics.iter().map(|diagnostic| to_binding_error(diagnostic, cwd.clone())).collect(),
    )
  }
}
