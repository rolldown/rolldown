use napi::{Unknown, bindgen_prelude::FromNapiValue};
use rolldown_plugin::__inner::{Pluginable, SharedPluginable};
use rolldown_plugin_bundle_analyzer::BundleAnalyzerPlugin;
use rolldown_plugin_esm_external_require::EsmExternalRequirePlugin;
use rolldown_plugin_isolated_declaration::IsolatedDeclarationPlugin;
use rolldown_plugin_oxc_runtime::OxcRuntimePlugin;
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
use rolldown_plugin_vite_web_worker_post::ViteWebWorkerPostPlugin;

use crate::options::plugin::config::{
  BindingBundleAnalyzerPluginConfig, BindingEsmExternalRequirePluginConfig,
  BindingViteModulePreloadPolyfillPluginConfig, BindingViteReactRefreshWrapperPluginConfig,
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

impl TryFrom<BindingBuiltinPlugin<'_>> for SharedPluginable {
  type Error = napi::Error;

  fn try_from(plugin: BindingBuiltinPlugin) -> Result<Self, Self::Error> {
    Ok(match plugin.__name {
      BindingBuiltinPluginName::BundleAnalyzer => {
        let plugin = if let Some(options) = plugin.options {
          BindingBundleAnalyzerPluginConfig::from_unknown(options)?.into()
        } else {
          BundleAnalyzerPlugin::default()
        };
        Pluginable::new_shared(plugin)
      }
      BindingBuiltinPluginName::EsmExternalRequire => {
        let plugin = if let Some(options) = plugin.options {
          BindingEsmExternalRequirePluginConfig::from_unknown(options)?.into()
        } else {
          EsmExternalRequirePlugin::default()
        };
        Pluginable::new_shared(plugin)
      }
      BindingBuiltinPluginName::IsolatedDeclaration => {
        let plugin = if let Some(options) = plugin.options {
          BindingIsolatedDeclarationPluginConfig::from_unknown(options)?.into()
        } else {
          IsolatedDeclarationPlugin::default()
        };
        Pluginable::new_shared(plugin)
      }
      BindingBuiltinPluginName::Replace => {
        let config = if let Some(options) = plugin.options {
          BindingReplacePluginConfig::from_unknown(options)?
        } else {
          BindingReplacePluginConfig::default()
        };
        Pluginable::new_shared(ReplacePlugin::with_options(config.try_into()?)?)
      }
      BindingBuiltinPluginName::ViteAlias => {
        let plugin = if let Some(options) = plugin.options {
          BindingViteAliasPluginConfig::from_unknown(options)?.try_into()?
        } else {
          ViteAliasPlugin::default()
        };
        Pluginable::new_shared(plugin)
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
        Pluginable::new_shared(ViteBuildImportAnalysisPlugin::try_from(config)?)
      }
      BindingBuiltinPluginName::ViteDynamicImportVars => {
        let plugin = if let Some(options) = plugin.options {
          BindingViteDynamicImportVarsPluginConfig::from_unknown(options)?.into()
        } else {
          ViteDynamicImportVarsPlugin::default()
        };
        Pluginable::new_shared(plugin)
      }
      BindingBuiltinPluginName::ViteImportGlob => {
        let plugin = if let Some(options) = plugin.options {
          BindingViteImportGlobPluginConfig::from_unknown(options)?.into()
        } else {
          ViteImportGlobPlugin::default()
        };
        Pluginable::new_shared(plugin)
      }
      BindingBuiltinPluginName::ViteJson => {
        let plugin = if let Some(options) = plugin.options {
          BindingViteJsonPluginConfig::from_unknown(options)?.try_into()?
        } else {
          ViteJsonPlugin::default()
        };
        Pluginable::new_shared(plugin)
      }
      BindingBuiltinPluginName::ViteLoadFallback => Pluginable::new_shared(ViteLoadFallbackPlugin),
      BindingBuiltinPluginName::ViteManifest => {
        let plugin: ViteManifestPlugin = if let Some(options) = plugin.options {
          BindingViteManifestPluginConfig::from_unknown(options)?.into()
        } else {
          return Err(napi::Error::new(
            napi::Status::InvalidArg,
            "Missing options for ViteManifestPlugin",
          ));
        };
        Pluginable::new_shared(plugin)
      }
      BindingBuiltinPluginName::ViteModulePreloadPolyfill => {
        let plugin = if let Some(options) = plugin.options {
          BindingViteModulePreloadPolyfillPluginConfig::from_unknown(options)?.into()
        } else {
          ViteModulePreloadPolyfillPlugin::default()
        };
        Pluginable::new_shared(plugin)
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
        Pluginable::new_shared(ViteReactRefreshWrapperPlugin::new(config.into()))
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
        Pluginable::new_shared(plugin)
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
        Pluginable::new_shared(ViteResolvePlugin::new(config.into()))
      }
      BindingBuiltinPluginName::ViteTransform => {
        let plugin = if let Some(options) = plugin.options {
          BindingViteTransformPluginConfig::from_unknown(options)?.into()
        } else {
          ViteTransformPlugin::default()
        };
        Pluginable::new_shared(plugin)
      }
      BindingBuiltinPluginName::ViteWebWorkerPost => {
        Pluginable::new_shared(ViteWebWorkerPostPlugin)
      }
      BindingBuiltinPluginName::OxcRuntime => Pluginable::new_shared(OxcRuntimePlugin),
    })
  }
}
