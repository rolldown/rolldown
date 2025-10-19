use napi_derive::napi;

use super::binding_hmr_boundary_output::BindingHmrBoundaryOutput;

#[napi(discriminant = "type", object_from_js = false)]
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
