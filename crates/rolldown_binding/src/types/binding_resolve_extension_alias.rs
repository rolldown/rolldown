#[napi_derive::napi(object)]
#[derive(Debug, Default)]
pub struct ExtensionAliasItem {
  pub target: String,
  pub replacements: Vec<String>,
}
