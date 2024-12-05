use serde::Deserialize;

#[napi_derive::napi(object)]
#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct BindingChecksOptions {
  pub circular_dependency: Option<bool>,
}

impl From<BindingChecksOptions> for rolldown_common::ChecksOptions {
  fn from(value: BindingChecksOptions) -> Self {
    Self { circular_dependency: value.circular_dependency }
  }
}
