use derivative::Derivative;
use napi::JsUnknown;
use napi_derive::napi;
use rolldown_plugin::Plugin;
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
}

impl From<BindingBuiltinPlugin> for Box<dyn Plugin> {
  fn from(plugin: BindingBuiltinPlugin) -> Self {
    match plugin.name {
      BindingBuiltinPluginName::WasmPlugin => Box::new(WasmPlugin {}),
    }
  }
}
