use derivative::Derivative;
use serde::Deserialize;

#[napi_derive::napi(object)]
#[derive(Deserialize, Default, Derivative)]
#[serde(rename_all = "camelCase")]
#[derivative(Debug)]
pub struct BindingHookResolveIdOutput {
  pub id: String,
  pub external: Option<bool>,
}

impl From<BindingHookResolveIdOutput> for rolldown_plugin::HookResolveIdOutput {
  fn from(value: BindingHookResolveIdOutput) -> Self {
    Self { id: value.id, external: value.external }
  }
}
