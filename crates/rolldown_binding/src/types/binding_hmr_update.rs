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
    has_skipped_boundary: bool,
    modules_to_update_count: u32,
  },
  FullReload {
    reason: Option<String>,
    has_skipped_boundary: bool,
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
        has_skipped_boundary: patch.has_skipped_boundary,
        modules_to_update_count: patch.modules_to_update_count,
      },
      rolldown_common::HmrUpdate::FullReload { reason, has_skipped_boundary } => {
        Self::FullReload { reason: Some(reason), has_skipped_boundary }
      }
      rolldown_common::HmrUpdate::Noop => Self::Noop,
    }
  }
}
