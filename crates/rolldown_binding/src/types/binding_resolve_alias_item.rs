#[napi_derive::napi(object)]
#[derive(Debug, Default)]
pub struct AliasItem {
  pub find: String,
  pub replacements: Vec<Option<String>>,
}
