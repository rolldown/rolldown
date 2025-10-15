#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug, Default)]
pub struct AliasItem {
  pub find: String,
  pub replacements: Vec<Option<String>>,
}
