use serde::Deserialize;

#[napi_derive::napi(object)]
#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct AliasItem {
  pub find: String,
  pub replacements: Vec<String>,
}
