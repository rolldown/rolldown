use serde::Deserialize;

#[napi_derive::napi(object)]
#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionAliasItem {
  pub target: String,
  pub replacements: Vec<String>,
}
