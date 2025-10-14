#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug, Default)]
pub struct ExtensionAliasItem {
  pub target: String,
  pub replacements: Vec<String>,
}
