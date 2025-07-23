use std::sync::Arc;

use napi::{Unknown, bindgen_prelude::FromNapiValue};
use napi_derive::napi;
use rolldown_plugin_manifest::ManifestPlugin;
use rolldown_plugin_oxc_runtime::OxcRuntimePlugin;

use rolldown_plugin::__inner::Pluginable;
use rolldown_plugin_alias::AliasPlugin;
use rolldown_plugin_asset::AssetPlugin;
use rolldown_plugin_asset_import_meta_url::AssetImportMetaUrlPlugin;
use rolldown_plugin_build_import_analysis::BuildImportAnalysisPlugin;
use rolldown_plugin_dynamic_import_vars::DynamicImportVarsPlugin;
use rolldown_plugin_import_glob::ImportGlobPlugin;
use rolldown_plugin_isolated_declaration::IsolatedDeclarationPlugin;
use rolldown_plugin_json::JsonPlugin;
use rolldown_plugin_load_fallback::LoadFallbackPlugin;
use rolldown_plugin_module_federation::ModuleFederationPlugin;
use rolldown_plugin_module_preload_polyfill::ModulePreloadPolyfillPlugin;
use rolldown_plugin_replace::ReplacePlugin;
use rolldown_plugin_reporter::ReporterPlugin;
use rolldown_plugin_transform::TransformPlugin;
use rolldown_plugin_vite_resolve::ViteResolvePlugin;
use rolldown_plugin_wasm_fallback::WasmFallbackPlugin;
use rolldown_plugin_wasm_helper::WasmHelperPlugin;
use rolldown_plugin_web_worker_post::WebWorkerPostPlugin;

use crate::options::plugin::config::{
  BindingModulePreloadPolyfillPluginConfig, BindingWasmHelperPluginConfig,
};

use super::{
  config::{
    BindingAliasPluginConfig, BindingAssetPluginConfig, BindingBuildImportAnalysisPluginConfig,
    BindingDynamicImportVarsPluginConfig, BindingImportGlobPluginConfig,
    BindingIsolatedDeclarationPluginConfig, BindingJsonPluginConfig, BindingManifestPluginConfig,
    BindingOxcRuntimePluginConfig, BindingReplacePluginConfig, BindingReporterPluginConfig,
    BindingTransformPluginConfig, BindingViteResolvePluginConfig,
  },
  types::{
    binding_builtin_plugin_name::BindingBuiltinPluginName,
    binding_module_federation_plugin_option::BindingModuleFederationPluginOption,
  },
};

#[allow(clippy::pub_underscore_fields)]
#[napi(object)]
pub struct BindingBuiltinPlugin<'a> {
  #[napi(js_name = "__name")]
  pub __name: BindingBuiltinPluginName,
  pub options: Option<Unknown<'a>>,
}

impl std::fmt::Debug for BindingBuiltinPlugin<'_> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("BindingBuiltinPlugin")
      .field("name", &self.__name)
      .field("options", &"<JsUnknown>")
      .finish()
  }
}

impl TryFrom<BindingBuiltinPlugin<'_>> for Arc<dyn Pluginable> {
  type Error = napi::Error;

  #[allow(clippy::too_many_lines)]
  fn try_from(plugin: BindingBuiltinPlugin) -> Result<Self, Self::Error> {
    Ok(match plugin.__name {
      BindingBuiltinPluginName::Alias => {
        let plugin = if let Some(options) = plugin.options {
          BindingAliasPluginConfig::from_unknown(options)?.try_into()?
        } else {
          AliasPlugin::default()
        };
        Arc::new(plugin)
      }
      BindingBuiltinPluginName::Asset => {
        let plugin = if let Some(options) = plugin.options {
          BindingAssetPluginConfig::from_unknown(options)?.into()
        } else {
          AssetPlugin::default()
        };
        Arc::new(plugin)
      }
      BindingBuiltinPluginName::AssetImportMetaUrl => Arc::new(AssetImportMetaUrlPlugin),
      BindingBuiltinPluginName::BuildImportAnalysis => {
        let config = if let Some(options) = plugin.options {
          BindingBuildImportAnalysisPluginConfig::from_unknown(options)?
        } else {
          return Err(napi::Error::new(
            napi::Status::InvalidArg,
            "Missing options for BuildImportAnalysisPlugin",
          ));
        };
        Arc::new(BuildImportAnalysisPlugin::try_from(config)?)
      }
      BindingBuiltinPluginName::DynamicImportVars => {
        let plugin = if let Some(options) = plugin.options {
          BindingDynamicImportVarsPluginConfig::from_unknown(options)?.into()
        } else {
          DynamicImportVarsPlugin::default()
        };
        Arc::new(plugin)
      }
      BindingBuiltinPluginName::ImportGlob => {
        let plugin = if let Some(options) = plugin.options {
          BindingImportGlobPluginConfig::from_unknown(options)?.into()
        } else {
          ImportGlobPlugin::default()
        };
        Arc::new(plugin)
      }
      BindingBuiltinPluginName::IsolatedDeclaration => {
        let plugin = if let Some(options) = plugin.options {
          BindingIsolatedDeclarationPluginConfig::from_unknown(options)?.into()
        } else {
          IsolatedDeclarationPlugin::default()
        };
        Arc::new(plugin)
      }
      BindingBuiltinPluginName::Json => {
        let plugin = if let Some(options) = plugin.options {
          BindingJsonPluginConfig::from_unknown(options)?.try_into()?
        } else {
          JsonPlugin::default()
        };
        Arc::new(plugin)
      }
      BindingBuiltinPluginName::LoadFallback => Arc::new(LoadFallbackPlugin),
      BindingBuiltinPluginName::Manifest => {
        let plugin = if let Some(options) = plugin.options {
          BindingManifestPluginConfig::from_unknown(options)?.into()
        } else {
          ManifestPlugin::default()
        };
        Arc::new(plugin)
      }
      BindingBuiltinPluginName::ModuleFederation => {
        let config = if let Some(options) = plugin.options {
          BindingModuleFederationPluginOption::from_unknown(options)?
        } else {
          return Err(napi::Error::new(
            napi::Status::InvalidArg,
            "Missing options for ModuleFederationPlugin",
          ));
        };
        Arc::new(ModuleFederationPlugin::new(config.into()))
      }
      BindingBuiltinPluginName::ModulePreloadPolyfill => {
        let plugin = if let Some(options) = plugin.options {
          BindingModulePreloadPolyfillPluginConfig::from_unknown(options)?.into()
        } else {
          ModulePreloadPolyfillPlugin::default()
        };
        Arc::new(plugin)
      }
      BindingBuiltinPluginName::OxcRuntime => {
        let plugin = if let Some(options) = plugin.options {
          BindingOxcRuntimePluginConfig::from_unknown(options)?.into()
        } else {
          OxcRuntimePlugin::default()
        };
        Arc::new(plugin)
      }
      BindingBuiltinPluginName::Report => {
        let plugin: ReporterPlugin = if let Some(options) = plugin.options {
          BindingReporterPluginConfig::from_unknown(options)?.into()
        } else {
          return Err(napi::Error::new(
            napi::Status::InvalidArg,
            "Missing options for ReportPlugin",
          ));
        };
        Arc::new(plugin)
      }
      BindingBuiltinPluginName::Replace => {
        let config = if let Some(options) = plugin.options {
          BindingReplacePluginConfig::from_unknown(options)?
        } else {
          BindingReplacePluginConfig::default()
        };
        Arc::new(ReplacePlugin::with_options(config.into()))
      }
      BindingBuiltinPluginName::Transform => {
        let plugin = if let Some(options) = plugin.options {
          BindingTransformPluginConfig::from_unknown(options)?.into()
        } else {
          TransformPlugin::default()
        };
        Arc::new(plugin)
      }
      BindingBuiltinPluginName::ViteResolve => {
        let config = if let Some(options) = plugin.options {
          BindingViteResolvePluginConfig::from_unknown(options)?
        } else {
          return Err(napi::Error::new(
            napi::Status::InvalidArg,
            "Missing options for ViteResolvePlugin",
          ));
        };
        Arc::new(ViteResolvePlugin::new(config.into()))
      }
      BindingBuiltinPluginName::WasmFallback => Arc::new(WasmFallbackPlugin),
      BindingBuiltinPluginName::WasmHelper => {
        let plugin = if let Some(options) = plugin.options {
          BindingWasmHelperPluginConfig::from_unknown(options)?.into()
        } else {
          WasmHelperPlugin::default()
        };
        Arc::new(plugin)
      }
      BindingBuiltinPluginName::WebWorkerPost => Arc::new(WebWorkerPostPlugin),
    })
  }
}
