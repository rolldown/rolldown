use std::sync::Arc;

use napi_derive::napi;
use rolldown_plugin::__inner::Pluginable;
use rolldown_plugin_glob_import::{GlobImportPlugin, GlobImportPluginConfig};
use rolldown_plugin_wasm::WasmPlugin;
use serde::Deserialize;

#[napi(object)]
#[derive(Debug)]
pub struct BindingBuiltinGlobImportPlugin {
  pub config: Option<BindingGlobImportPluginConfig>,
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

impl From<BindingBuiltinGlobImportPlugin> for Arc<dyn Pluginable> {
  fn from(value: BindingBuiltinGlobImportPlugin) -> Self {
    Arc::new(GlobImportPlugin { config: value.config.map(Into::into).unwrap_or_default() })
  }
}

#[napi(object)]
#[derive(Debug)]
pub struct BindingBuiltinWasmPlugin {}
impl From<BindingBuiltinWasmPlugin> for Arc<dyn Pluginable> {
  fn from(_: BindingBuiltinWasmPlugin) -> Self {
    Arc::new(WasmPlugin {})
  }
}

// fn try_convert_builting_plugin_options(
//   options: Option<JsObject>,
// ) -> Result<GlobImportPluginConfig, napi::Error> {
//   let mut config = GlobImportPluginConfig::default();
//   match options {
//     Some(options) => {
//       // let obj: BindingGlobImportPluginConfig = options.&mut();
//       // from_js_object(obj, &mut config)?;
//       // if options.has_named_property("root")? {
//       //   let root = options.get_named_property::<JsString>("root")?.into_utf8()?;
//       //   config.root = Some(root.as_str()?.to_string());
//       // }
//       // if options.has_named_property("restoreQueryExtension")? {
//       //   let root = options.get_named_property::<JsBoolean>("restoreQueryExtension")?;
//       //   config.restore_query_extension = root.get_value()?;
//       // }
//     }
//     None => {}
//   }
//   Ok(config)
// }
