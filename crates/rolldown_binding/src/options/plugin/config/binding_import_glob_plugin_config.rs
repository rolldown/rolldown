use rolldown_plugin_import_glob::ImportGlobPluginConfig;

#[napi_derive::napi(object)]
#[derive(Debug, Default)]
pub struct BindingImportGlobPluginConfig {
  pub root: Option<String>,
  pub restore_query_extension: Option<bool>,
}

impl From<BindingImportGlobPluginConfig> for ImportGlobPluginConfig {
  fn from(value: BindingImportGlobPluginConfig) -> Self {
    ImportGlobPluginConfig {
      root: value.root,
      restore_query_extension: value.restore_query_extension.unwrap_or_default(),
    }
  }
}
