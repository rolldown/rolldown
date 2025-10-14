#[napi_derive::napi(object, object_from_js = false)]
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
