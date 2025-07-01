use rolldown_plugin_wasm_helper::WasmHelperPlugin;

#[napi_derive::napi(object)]
#[derive(Debug, Default)]
pub struct BindingWasmHelperPluginConfig {
  pub decoded_base: String,
}

impl From<BindingWasmHelperPluginConfig> for WasmHelperPlugin {
  fn from(value: BindingWasmHelperPluginConfig) -> Self {
    Self { decoded_base: value.decoded_base }
  }
}
