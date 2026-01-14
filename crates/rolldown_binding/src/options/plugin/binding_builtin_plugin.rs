use std::sync::Arc;

use napi::{Unknown, bindgen_prelude::FromNapiValue};
use rolldown_plugin::__inner::Pluginable;
use rolldown_plugin_esm_external_require::EsmExternalRequirePlugin;
use rolldown_plugin_isolated_declaration::IsolatedDeclarationPlugin;
use rolldown_plugin_replace::ReplacePlugin;
use rolldown_plugin_vite_alias::ViteAliasPlugin;
use rolldown_plugin_vite_build_import_analysis::ViteBuildImportAnalysisPlugin;
use rolldown_plugin_vite_dynamic_import_vars::ViteDynamicImportVarsPlugin;
use rolldown_plugin_vite_import_glob::ViteImportGlobPlugin;
use rolldown_plugin_vite_json::ViteJsonPlugin;
use rolldown_plugin_vite_load_fallback::ViteLoadFallbackPlugin;
use rolldown_plugin_vite_manifest::ViteManifestPlugin;
use rolldown_plugin_vite_module_preload_polyfill::ViteModulePreloadPolyfillPlugin;
use rolldown_plugin_vite_react_refresh_wrapper::ViteReactRefreshWrapperPlugin;
use rolldown_plugin_vite_reporter::ViteReporterPlugin;
use rolldown_plugin_vite_resolve::ViteResolvePlugin;
use rolldown_plugin_vite_transform::ViteTransformPlugin;
use rolldown_plugin_vite_wasm_fallback::ViteWasmFallbackPlugin;
use rolldown_plugin_vite_wasm_helper::ViteWasmHelperPlugin;
use rolldown_plugin_vite_web_worker_post::ViteWebWorkerPostPlugin;

use crate::options::plugin::config::{
  BindingEsmExternalRequirePluginConfig, BindingViteModulePreloadPolyfillPluginConfig,
  BindingViteReactRefreshWrapperPluginConfig, BindingViteWasmHelperPluginConfig,
};

use super::{
  config::{
    BindingIsolatedDeclarationPluginConfig, BindingReplacePluginConfig,
    BindingViteAliasPluginConfig, BindingViteBuildImportAnalysisPluginConfig,
    BindingViteDynamicImportVarsPluginConfig, BindingViteImportGlobPluginConfig,
    BindingViteJsonPluginConfig, BindingViteManifestPluginConfig, BindingViteReporterPluginConfig,
    BindingViteResolvePluginConfig, BindingViteTransformPluginConfig,
  },
  types::binding_builtin_plugin_name::BindingBuiltinPluginName,
};

#[expect(clippy::pub_underscore_fields)]
#[napi_derive::napi(object, object_to_js = false)]
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

  fn try_from(plugin: BindingBuiltinPlugin) -> Result<Self, Self::Error> {
    Ok(match plugin.__name {
      BindingBuiltinPluginName::EsmExternalRequire => {
        let plugin = if let Some(options) = plugin.options {
          BindingEsmExternalRequirePluginConfig::from_unknown(options)?.into()
        } else {
          EsmExternalRequirePlugin::default()
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
      BindingBuiltinPluginName::Replace => {
        let config = if let Some(options) = plugin.options {
          BindingReplacePluginConfig::from_unknown(options)?
        } else {
          BindingReplacePluginConfig::default()
        };
        Arc::new(ReplacePlugin::with_options(config.into())?)
      }
      BindingBuiltinPluginName::ViteAlias => {
        let plugin = if let Some(options) = plugin.options {
          BindingViteAliasPluginConfig::from_unknown(options)?.try_into()?
        } else {
          ViteAliasPlugin::default()
        };
        Arc::new(plugin)
      }
      BindingBuiltinPluginName::ViteBuildImportAnalysis => {
        let config = if let Some(options) = plugin.options {
          BindingViteBuildImportAnalysisPluginConfig::from_unknown(options)?
        } else {
          return Err(napi::Error::new(
            napi::Status::InvalidArg,
            "Missing options for ViteBuildImportAnalysisPlugin",
          ));
        };
        Arc::new(ViteBuildImportAnalysisPlugin::try_from(config)?)
      }
      BindingBuiltinPluginName::ViteDynamicImportVars => {
        let plugin = if let Some(options) = plugin.options {
          BindingViteDynamicImportVarsPluginConfig::from_unknown(options)?.into()
        } else {
          ViteDynamicImportVarsPlugin::default()
        };
        Arc::new(plugin)
      }
      BindingBuiltinPluginName::ViteImportGlob => {
        let plugin = if let Some(options) = plugin.options {
          BindingViteImportGlobPluginConfig::from_unknown(options)?.into()
        } else {
          ViteImportGlobPlugin::default()
        };
        Arc::new(plugin)
      }
      BindingBuiltinPluginName::ViteJson => {
        let plugin = if let Some(options) = plugin.options {
          BindingViteJsonPluginConfig::from_unknown(options)?.try_into()?
        } else {
          ViteJsonPlugin::default()
        };
        Arc::new(plugin)
      }
      BindingBuiltinPluginName::ViteLoadFallback => Arc::new(ViteLoadFallbackPlugin),
      BindingBuiltinPluginName::ViteManifest => {
        let plugin: ViteManifestPlugin = if let Some(options) = plugin.options {
          BindingViteManifestPluginConfig::from_unknown(options)?.into()
        } else {
          return Err(napi::Error::new(
            napi::Status::InvalidArg,
            "Missing options for ViteManifestPlugin",
          ));
        };
        Arc::new(plugin)
      }
      BindingBuiltinPluginName::ViteModulePreloadPolyfill => {
        let plugin = if let Some(options) = plugin.options {
          BindingViteModulePreloadPolyfillPluginConfig::from_unknown(options)?.into()
        } else {
          ViteModulePreloadPolyfillPlugin::default()
        };
        Arc::new(plugin)
      }
      BindingBuiltinPluginName::ViteReactRefreshWrapper => {
        let config = if let Some(options) = plugin.options {
          BindingViteReactRefreshWrapperPluginConfig::from_unknown(options)?
        } else {
          return Err(napi::Error::new(
            napi::Status::InvalidArg,
            "Missing options for ViteReactRefreshWrapperPlugin",
          ));
        };
        Arc::new(ViteReactRefreshWrapperPlugin::new(config.into()))
      }
      BindingBuiltinPluginName::ViteReporter => {
        let plugin: ViteReporterPlugin = if let Some(options) = plugin.options {
          BindingViteReporterPluginConfig::from_unknown(options)?.into()
        } else {
          return Err(napi::Error::new(
            napi::Status::InvalidArg,
            "Missing options for ViteReporterPlugin",
          ));
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
      BindingBuiltinPluginName::ViteTransform => {
        let plugin = if let Some(options) = plugin.options {
          BindingViteTransformPluginConfig::from_unknown(options)?.into()
        } else {
          ViteTransformPlugin::default()
        };
        Arc::new(plugin)
      }
      BindingBuiltinPluginName::ViteWasmFallback => Arc::new(ViteWasmFallbackPlugin),
      BindingBuiltinPluginName::ViteWasmHelper => {
        let plugin = if let Some(options) = plugin.options {
          BindingViteWasmHelperPluginConfig::from_unknown(options)?.into()
        } else {
          ViteWasmHelperPlugin::default()
        };
        Arc::new(plugin)
      }
      BindingBuiltinPluginName::ViteWebWorkerPost => Arc::new(ViteWebWorkerPostPlugin),
    })
  }
}
