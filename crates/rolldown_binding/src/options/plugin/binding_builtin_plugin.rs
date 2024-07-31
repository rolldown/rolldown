use derivative::Derivative;
use napi::bindgen_prelude::FromNapiValue;
use napi::JsUnknown;
use napi_derive::napi;
use rolldown_plugin::__inner::Pluginable;
use rolldown_plugin_glob_import::{GlobImportPlugin, GlobImportPluginConfig};
use rolldown_plugin_wasm::WasmPlugin;
use serde::Deserialize;
use std::sync::Arc;

#[allow(clippy::pub_underscore_fields)]
#[napi(object)]
#[derive(Deserialize, Derivative)]
pub struct BindingBuiltinPlugin {
  #[napi(js_name = "__name")]
  pub __name: BindingBuiltinPluginName,
  #[serde(skip_deserializing)]
  pub options: Option<JsUnknown>,
}

impl std::fmt::Debug for BindingBuiltinPlugin {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("BindingBuiltinPlugin")
      .field("name", &self.__name)
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

#[napi_derive::napi(object)]
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BindingGlobImportPluginConfig {
  pub root: Option<String>,
  pub restore_query_extension: Option<bool>,
}

impl From<BindingGlobImportPluginConfig> for GlobImportPluginConfig {
  fn from(value: BindingGlobImportPluginConfig) -> Self {
    GlobImportPluginConfig {
      root: value.root,
      restore_query_extension: value.restore_query_extension.unwrap_or_default(),
    }
  }
}

impl TryFrom<BindingBuiltinPlugin> for Arc<dyn Pluginable> {
  type Error = napi::Error;

  fn try_from(plugin: BindingBuiltinPlugin) -> Result<Self, Self::Error> {
    Ok(match plugin.__name {
      BindingBuiltinPluginName::WasmPlugin => Arc::new(WasmPlugin {}),
      BindingBuiltinPluginName::GlobImportPlugin => {
        let config = if let Some(options) = plugin.options {
          BindingGlobImportPluginConfig::from_unknown(options)?.into()
        } else {
          GlobImportPluginConfig::default()
        };
        Arc::new(GlobImportPlugin { config })
      }
    })
  }
}
