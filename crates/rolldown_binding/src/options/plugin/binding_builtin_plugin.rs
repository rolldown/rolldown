use derivative::Derivative;
use napi::bindgen_prelude::FromNapiValue;
use napi::JsUnknown;
use napi_derive::napi;
use rolldown_plugin::__inner::Pluginable;
use rolldown_plugin_dynamic_import_vars::DynamicImportVarsPlugin;
use rolldown_plugin_glob_import::{GlobImportPlugin, GlobImportPluginConfig};
use rolldown_plugin_load_fallback::LoadFallbackPlugin;
use rolldown_plugin_manifest::{ManifestPlugin, ManifestPluginConfig};
use rolldown_plugin_module_preload_polyfill::ModulePreloadPolyfillPlugin;
use rolldown_plugin_transform::{StringOrRegex, TransformPlugin};
use rolldown_plugin_wasm::WasmPlugin;
use serde::Deserialize;
use std::sync::Arc;

use super::types::binding_js_or_regex::BindingStringOrRegex;

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

#[allow(clippy::enum_variant_names)]
#[derive(Debug, Deserialize)]
#[napi]
pub enum BindingBuiltinPluginName {
  WasmPlugin,
  GlobImportPlugin,
  DynamicImportVarsPlugin,
  ModulePreloadPolyfillPlugin,
  ManifestPlugin,
  LoadFallbackPlugin,
  TransformPlugin,
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

#[napi_derive::napi(object)]
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BindingManifestPluginConfig {
  pub root: String,
  pub out_path: String,
  // TODO: Link this with assets plugin
  // pub generated_assets: Option<Map<String,  GeneratedAssetMeta>>,
}

impl From<BindingManifestPluginConfig> for ManifestPluginConfig {
  fn from(value: BindingManifestPluginConfig) -> Self {
    ManifestPluginConfig { root: value.root, out_path: value.out_path }
  }
}

#[napi_derive::napi(object)]
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BindingModulePreloadPolyfillPluginConfig {
  pub skip: Option<bool>,
}

#[napi_derive::napi(object)]
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BindingTransformPluginConfig {
  pub include: Option<Vec<BindingStringOrRegex>>,
  pub exclude: Option<Vec<BindingStringOrRegex>>,
  pub jsx_inject: Option<String>,
}

impl TryFrom<BindingTransformPluginConfig> for TransformPlugin {
  type Error = anyhow::Error;

  fn try_from(value: BindingTransformPluginConfig) -> Result<Self, Self::Error> {
    let normalized_include = if let Some(include) = value.include {
      let mut ret = Vec::with_capacity(include.len());
      for item in include {
        ret.push(StringOrRegex::new(item.value, &item.flag)?);
      }
      ret
    } else {
      vec![]
    };
    let normalized_exclude = if let Some(exclude) = value.exclude {
      let mut ret = Vec::with_capacity(exclude.len());
      for item in exclude {
        ret.push(StringOrRegex::new(item.value, &item.flag)?);
      }
      ret
    } else {
      vec![]
    };
    Ok(TransformPlugin {
      include: normalized_include,
      exclude: normalized_exclude,
      jsx_inject: value.jsx_inject,
    })
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
      BindingBuiltinPluginName::DynamicImportVarsPlugin => Arc::new(DynamicImportVarsPlugin {}),
      BindingBuiltinPluginName::ModulePreloadPolyfillPlugin => {
        let skip = if let Some(options) = plugin.options {
          let config = BindingModulePreloadPolyfillPluginConfig::from_unknown(options)?;
          config.skip.unwrap_or_default()
        } else {
          false
        };
        Arc::new(ModulePreloadPolyfillPlugin { skip })
      }
      BindingBuiltinPluginName::ManifestPlugin => {
        let config = if let Some(options) = plugin.options {
          BindingManifestPluginConfig::from_unknown(options)?.into()
        } else {
          ManifestPluginConfig::default()
        };
        Arc::new(ManifestPlugin { config })
      }
      BindingBuiltinPluginName::LoadFallbackPlugin => Arc::new(LoadFallbackPlugin {}),
      BindingBuiltinPluginName::TransformPlugin => {
        let plugin = if let Some(options) = plugin.options {
          BindingTransformPluginConfig::from_unknown(options)?.try_into()?
        } else {
          TransformPlugin::default()
        };
        Arc::new(plugin)
      }
    })
  }
}
