#[napi_derive::napi(object)]
#[derive(Debug)]
pub struct BindingHmrOutput {
  pub patch: String,
  pub hmr_boundaries: Vec<BindingHmrBoundaryOutput>,
  pub full_reload: bool,
}

impl From<rolldown_common::HmrOutput> for BindingHmrOutput {
  fn from(value: rolldown_common::HmrOutput) -> Self {
    Self {
      patch: value.patch,
      hmr_boundaries: value.hmr_boundaries.into_iter().map(Into::into).collect(),
      full_reload: value.full_reload,
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
