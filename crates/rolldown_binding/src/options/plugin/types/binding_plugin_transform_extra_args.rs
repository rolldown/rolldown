use serde::Deserialize;

#[napi_derive::napi(object)]
#[derive(Deserialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BindingTransformHookExtraArgs {
  pub module_type: String,
}
