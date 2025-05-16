#[napi_derive::napi(object)]
#[derive(Debug)]
pub struct BindingHmrOutput {
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

impl From<rolldown_common::HmrOutput> for BindingHmrOutput {
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
