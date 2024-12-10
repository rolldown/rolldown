#[napi_derive::napi(object)]
#[derive(Debug, Default)]
pub struct BindingChecksOptions {
  pub circular_dependency: Option<bool>,
}

impl From<BindingChecksOptions> for rolldown_common::ChecksOptions {
  fn from(value: BindingChecksOptions) -> Self {
    Self { circular_dependency: value.circular_dependency }
  }
}
