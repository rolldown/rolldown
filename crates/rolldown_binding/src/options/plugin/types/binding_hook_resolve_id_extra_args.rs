use derivative::Derivative;
use serde::Deserialize;

#[napi_derive::napi(object)]
#[derive(Deserialize, Default, Derivative)]
#[serde(rename_all = "camelCase")]
#[derivative(Debug)]
pub struct BindingHookResolveIdExtraArgs {
  pub custom: Option<u32>,
  pub is_entry: bool,
  #[napi(ts_type = "'import' | 'dynamic-import' | 'require-call'")]
  pub kind: String,
}
