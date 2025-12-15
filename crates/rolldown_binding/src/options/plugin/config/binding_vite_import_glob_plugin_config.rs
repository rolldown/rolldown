use rolldown_plugin_vite_import_glob::{ViteImportGlobPlugin, ViteImportGlobPluginV2Config};

#[napi_derive::napi(object)]
#[derive(Debug, Default)]
pub struct BindingViteImportGlobPluginV2Config {
  pub sourcemap: Option<bool>,
}

#[napi_derive::napi(object)]
#[derive(Debug, Default)]
pub struct BindingViteImportGlobPluginConfig {
  pub root: Option<String>,
  pub restore_query_extension: Option<bool>,
  pub is_v2: Option<BindingViteImportGlobPluginV2Config>,
}

impl From<BindingViteImportGlobPluginConfig> for ViteImportGlobPlugin {
  fn from(value: BindingViteImportGlobPluginConfig) -> Self {
    Self {
      root: value.root,
      restore_query_extension: value.restore_query_extension.unwrap_or_default(),
      is_v2: value
        .is_v2
        .map(|v2| ViteImportGlobPluginV2Config { sourcemap: v2.sourcemap.unwrap_or_default() }),
    }
  }
}
