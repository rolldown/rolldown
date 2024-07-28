use std::sync::Arc;

use derivative::Derivative;
use napi::JsUnknown;
use napi_derive::napi;
use rolldown_plugin::__inner::Pluginable;
use rolldown_plugin_glob_import::GlobImportPlugin;
use rolldown_plugin_wasm::WasmPlugin;
use serde::Deserialize;

#[napi(object)]
#[derive(Deserialize, Derivative)]
pub struct BindingBuiltinPlugin {
  pub name: BindingBuiltinPluginName,
  #[serde(skip_deserializing)]
  pub options: Option<JsUnknown>,
}

impl std::fmt::Debug for BindingBuiltinPlugin {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("BindingBuiltinPlugin")
      .field("name", &self.name)
      .field("options", &"<JsUnknown>")
      .finish()
  }
}

#[derive(Debug, Deserialize)]
#[napi]
pub enum BindingBuiltinPluginName {
  WasmPlugin,
  GlobImportPlugin,
}

impl From<BindingBuiltinPlugin> for Arc<dyn Pluginable> {
  fn from(plugin: BindingBuiltinPlugin) -> Self {
    match plugin.name {
      BindingBuiltinPluginName::WasmPlugin => Arc::new(WasmPlugin {}),
      BindingBuiltinPluginName::GlobImportPlugin => Arc::new(GlobImportPlugin {}),
    }
  }
}
