use derivative::Derivative;
use serde::Deserialize;

#[napi_derive::napi(object)]
#[derive(Deserialize, Default, Derivative)]
#[serde(rename_all = "camelCase")]
#[derivative(Debug)]
pub struct BindingHookResolveIdExtraOptions {
  pub is_entry: bool,
  #[napi(ts_type = "'import' | 'dynamic-import' | 'require-call'")]
  pub kind: String,
}

impl From<rolldown_plugin::HookResolveIdExtraOptions> for BindingHookResolveIdExtraOptions {
  fn from(value: rolldown_plugin::HookResolveIdExtraOptions) -> Self {
    Self { is_entry: value.is_entry, kind: value.kind.to_string() }
  }
}
