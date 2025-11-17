use rolldown_plugin_vite_import_glob::{ViteImportGlobPlugin, ViteImportGlobPluginConfig};

#[napi_derive::napi(object)]
#[derive(Debug, Default)]
pub struct BindingViteImportGlobPluginConfig {
  pub root: Option<String>,
  pub restore_query_extension: Option<bool>,
}

impl From<BindingViteImportGlobPluginConfig> for ViteImportGlobPlugin {
  fn from(value: BindingViteImportGlobPluginConfig) -> Self {
    Self {
      config: ViteImportGlobPluginConfig {
        root: value.root,
        restore_query_extension: value.restore_query_extension.unwrap_or_default(),
      },
    }
  }
}
