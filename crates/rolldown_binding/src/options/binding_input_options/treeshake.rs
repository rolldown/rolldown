use rolldown::{InnerOptions, TreeshakeOptions};
use serde::Deserialize;

#[napi_derive::napi(object)]
#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct BindingTreeshake {
  pub module_side_effects: bool,
}

impl From<BindingTreeshake> for rolldown::TreeshakeOptions {
  fn from(value: BindingTreeshake) -> Self {
    Self::Option(InnerOptions { module_side_effects: value.module_side_effects })
  }
}
