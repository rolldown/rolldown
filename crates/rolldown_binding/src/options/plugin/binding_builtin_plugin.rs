use derivative::Derivative;
use napi::bindgen_prelude::FromNapiValue;
use napi::JsUnknown;
use napi_derive::napi;
use rolldown_plugin::__inner::Pluginable;
use rolldown_plugin_alias::{Alias, AliasPlugin};
use rolldown_plugin_build_import_analysis::BuildImportAnalysisPlugin;
use rolldown_plugin_dynamic_import_vars::DynamicImportVarsPlugin;
use rolldown_plugin_import_glob::{ImportGlobPlugin, ImportGlobPluginConfig};
use rolldown_plugin_json::JsonPlugin;
use rolldown_plugin_load_fallback::LoadFallbackPlugin;
use rolldown_plugin_manifest::{ManifestPlugin, ManifestPluginConfig};
use rolldown_plugin_module_preload_polyfill::ModulePreloadPolyfillPlugin;
use rolldown_plugin_replace::{ReplaceOptions, ReplacePlugin};
use rolldown_plugin_transform::TransformPlugin;
use rolldown_plugin_wasm_fallback::WasmFallbackPlugin;
use rolldown_plugin_wasm_helper::WasmHelperPlugin;
use rolldown_utils::pattern_filter::StringOrRegex;
use serde::Deserialize;
use std::{collections::HashMap, sync::Arc};

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
  WasmHelperPlugin,
  ImportGlobPlugin,
  DynamicImportVarsPlugin,
  ModulePreloadPolyfillPlugin,
  ManifestPlugin,
  LoadFallbackPlugin,
  TransformPlugin,
  WasmFallbackPlugin,
  AliasPlugin,
  JsonPlugin,
  BuildImportAnalysisPlugin,
  ReplacePlugin,
}

#[napi_derive::napi(object)]
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BindingGlobImportPluginConfig {
  pub root: Option<String>,
  pub restore_query_extension: Option<bool>,
}

impl From<BindingGlobImportPluginConfig> for ImportGlobPluginConfig {
  fn from(value: BindingGlobImportPluginConfig) -> Self {
    ImportGlobPluginConfig {
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
pub struct BindingJsonPluginConfig {
  pub stringify: Option<bool>,
  pub is_build: Option<bool>,
}

#[napi_derive::napi(object)]
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BindingTransformPluginConfig {
  pub include: Option<Vec<BindingStringOrRegex>>,
  pub exclude: Option<Vec<BindingStringOrRegex>>,
  pub jsx_inject: Option<String>,
  pub targets: Option<String>,
}

#[napi_derive::napi(object)]
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BindingAliasPluginConfig {
  pub entries: Vec<BindingAliasPluginAlias>,
}

#[napi_derive::napi(object)]
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BindingAliasPluginAlias {
  pub find: BindingStringOrRegex,
  pub replacement: String,
}

#[napi_derive::napi(object)]
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::struct_excessive_bools)]
pub struct BindingBuildImportAnalysisPluginConfig {
  pub preload_code: String,
  pub insert_preload: bool,
  pub optimize_module_preload_relative_paths: bool,
  pub render_built_url: bool,
  pub is_relative_base: bool,
}

impl TryFrom<BindingBuildImportAnalysisPluginConfig> for BuildImportAnalysisPlugin {
  type Error = anyhow::Error;

  fn try_from(value: BindingBuildImportAnalysisPluginConfig) -> Result<Self, Self::Error> {
    Ok(BuildImportAnalysisPlugin {
      preload_code: value.preload_code,
      insert_preload: value.insert_preload,
      render_built_url: value.render_built_url,
      is_relative_base: value.is_relative_base,
    })
  }
}

impl TryFrom<BindingAliasPluginConfig> for AliasPlugin {
  type Error = anyhow::Error;

  fn try_from(value: BindingAliasPluginConfig) -> Result<Self, Self::Error> {
    let mut ret = Vec::with_capacity(value.entries.len());
    for item in value.entries {
      ret.push(Alias {
        find: StringOrRegex::new(item.find.value, &item.find.flag)?,
        replacement: item.replacement,
      });
    }

    Ok(Self { entries: ret })
  }
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
      targets: value.targets,
    })
  }
}

impl TryFrom<BindingBuiltinPlugin> for Arc<dyn Pluginable> {
  type Error = napi::Error;

  fn try_from(plugin: BindingBuiltinPlugin) -> Result<Self, Self::Error> {
    Ok(match plugin.__name {
      BindingBuiltinPluginName::WasmHelperPlugin => Arc::new(WasmHelperPlugin {}),
      BindingBuiltinPluginName::WasmFallbackPlugin => Arc::new(WasmFallbackPlugin {}),
      BindingBuiltinPluginName::ImportGlobPlugin => {
        let config = if let Some(options) = plugin.options {
          BindingGlobImportPluginConfig::from_unknown(options)?.into()
        } else {
          ImportGlobPluginConfig::default()
        };
        Arc::new(ImportGlobPlugin { config })
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
      BindingBuiltinPluginName::AliasPlugin => {
        let plugin = if let Some(options) = plugin.options {
          BindingAliasPluginConfig::from_unknown(options)?.try_into()?
        } else {
          AliasPlugin::default()
        };
        Arc::new(plugin)
      }

      BindingBuiltinPluginName::JsonPlugin => {
        let config = if let Some(options) = plugin.options {
          BindingJsonPluginConfig::from_unknown(options)?
        } else {
          BindingJsonPluginConfig::default()
        };
        Arc::new(JsonPlugin {
          stringify: config.stringify.unwrap_or_default(),
          is_build: config.is_build.unwrap_or_default(),
        })
      }
      BindingBuiltinPluginName::BuildImportAnalysisPlugin => {
        let config: BindingBuildImportAnalysisPluginConfig = if let Some(options) = plugin.options {
          BindingBuildImportAnalysisPluginConfig::from_unknown(options)?
        } else {
          return Err(napi::Error::new(
            napi::Status::InvalidArg,
            "Missing options for BuildImportAnalysisPlugin",
          ));
        };
        Arc::new(BuildImportAnalysisPlugin::try_from(config)?)
      }
      BindingBuiltinPluginName::ReplacePlugin => {
        let config = if let Some(options) = plugin.options {
          Some(BindingReplacePluginConfig::from_unknown(options)?)
        } else {
          None
        };

        Arc::new(ReplacePlugin::with_options(config.map_or_else(ReplaceOptions::default, |opts| {
          ReplaceOptions {
            values: opts.values,
            delimiters: opts.delimiters.map_or_else(
              || ReplaceOptions::default().delimiters,
              |raw| (raw[0].clone(), raw[1].clone()),
            ),
            prevent_assignment: opts.prevent_assignment.unwrap_or(false),
            object_guards: opts.object_guards.unwrap_or(false),
          }
        })))
      }
    })
  }
}

#[napi_derive::napi(object)]
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BindingReplacePluginConfig {
  // It's ok we use `HashMap` here, because we don't care about the order of the keys.
  pub values: HashMap<String, String>,
  #[napi(ts_type = "[string, string]")]
  pub delimiters: Option<Vec<String>>,
  pub prevent_assignment: Option<bool>,
  pub object_guards: Option<bool>,
}
