use rolldown_common::JsModuleMetaEntry;

/// @internal One top-level key of the `meta` of a hook result, as the persistent cache saves it.
#[napi_derive::napi(object)]
#[derive(Debug)]
pub struct BindingModuleMetaEntry {
  pub key: String,
  /// The JSON text of the value. Not set for `undefined`, which has no JSON text.
  pub json: Option<String>,
}

impl From<BindingModuleMetaEntry> for JsModuleMetaEntry {
  fn from(entry: BindingModuleMetaEntry) -> Self {
    Self { key: entry.key, json: entry.json }
  }
}
