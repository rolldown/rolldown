use std::sync::Arc;

use napi::{Unknown, bindgen_prelude::FromNapiValue};
use rolldown_plugin::__inner::Pluginable;
use rolldown_plugin_dynamic_import_vars::DynamicImportVarsPlugin;
use rolldown_plugin_esm_external_require::EsmExternalRequirePlugin;
use rolldown_plugin_html_inline_proxy::HtmlInlineProxyPlugin;
use rolldown_plugin_import_glob::ImportGlobPlugin;
use rolldown_plugin_isolated_declaration::IsolatedDeclarationPlugin;
use rolldown_plugin_json::JsonPlugin;
use rolldown_plugin_load_fallback::LoadFallbackPlugin;
use rolldown_plugin_manifest::ManifestPlugin;
use rolldown_plugin_module_preload_polyfill::ModulePreloadPolyfillPlugin;
use rolldown_plugin_react_refresh_wrapper::ReactRefreshWrapperPlugin;
use rolldown_plugin_replace::ReplacePlugin;
use rolldown_plugin_reporter::ReporterPlugin;
use rolldown_plugin_transform::TransformPlugin;
use rolldown_plugin_vite_alias::ViteAliasPlugin;
use rolldown_plugin_vite_asset::ViteAssetPlugin;
use rolldown_plugin_vite_asset_import_meta_url::ViteAssetImportMetaUrlPlugin;
use rolldown_plugin_vite_build_import_analysis::ViteBuildImportAnalysisPlugin;
use rolldown_plugin_vite_css::ViteCSSPlugin;
use rolldown_plugin_vite_css_post::ViteCSSPostPlugin;
use rolldown_plugin_vite_html::ViteHtmlPlugin;
use rolldown_plugin_vite_resolve::ViteResolvePlugin;
use rolldown_plugin_wasm_fallback::WasmFallbackPlugin;
use rolldown_plugin_wasm_helper::WasmHelperPlugin;
use rolldown_plugin_web_worker_post::WebWorkerPostPlugin;

use crate::options::plugin::config::{
  BindingEsmExternalRequirePluginConfig, BindingHtmlInlineProxyPluginConfig,
  BindingModulePreloadPolyfillPluginConfig, BindingReactRefreshWrapperPluginConfig,
  BindingViteCSSPluginConfig, BindingViteCSSPostPluginConfig, BindingViteHtmlPluginConfig,
  BindingWasmHelperPluginConfig,
};

use super::{
  config::{
    BindingDynamicImportVarsPluginConfig, BindingImportGlobPluginConfig,
    BindingIsolatedDeclarationPluginConfig, BindingJsonPluginConfig, BindingManifestPluginConfig,
    BindingReplacePluginConfig, BindingReporterPluginConfig, BindingTransformPluginConfig,
    BindingViteAliasPluginConfig, BindingViteAssetPluginConfig,
    BindingViteBuildImportAnalysisPluginConfig, BindingViteResolvePluginConfig,
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
      BindingBuiltinPluginName::DynamicImportVars => {
        let plugin = if let Some(options) = plugin.options {
          BindingDynamicImportVarsPluginConfig::from_unknown(options)?.into()
        } else {
          DynamicImportVarsPlugin::default()
        };
        Arc::new(plugin)
      }
      BindingBuiltinPluginName::EsmExternalRequire => {
        let plugin = if let Some(options) = plugin.options {
          BindingEsmExternalRequirePluginConfig::from_unknown(options)?.into()
        } else {
          EsmExternalRequirePlugin::default()
        };
        Arc::new(plugin)
      }
      BindingBuiltinPluginName::HtmlInlineProxy => {
        let plugin: HtmlInlineProxyPlugin = if let Some(options) = plugin.options {
          BindingHtmlInlineProxyPluginConfig::from_unknown(options)?.into()
        } else {
          return Err(napi::Error::new(
            napi::Status::InvalidArg,
            "Missing options for HtmlInlineProxyPlugin",
          ));
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
        let plugin: ManifestPlugin = if let Some(options) = plugin.options {
          BindingManifestPluginConfig::from_unknown(options)?.into()
        } else {
          return Err(napi::Error::new(
            napi::Status::InvalidArg,
            "Missing options for ManifestPlugin",
          ));
        };
        Arc::new(plugin)
      }
      BindingBuiltinPluginName::ModulePreloadPolyfill => {
        let plugin = if let Some(options) = plugin.options {
          BindingModulePreloadPolyfillPluginConfig::from_unknown(options)?.into()
        } else {
          ModulePreloadPolyfillPlugin::default()
        };
        Arc::new(plugin)
      }
      BindingBuiltinPluginName::ReactRefreshWrapper => {
        let config = if let Some(options) = plugin.options {
          BindingReactRefreshWrapperPluginConfig::from_unknown(options)?
        } else {
          return Err(napi::Error::new(
            napi::Status::InvalidArg,
            "Missing options for ReactRefreshWrapperPlugin",
          ));
        };
        Arc::new(ReactRefreshWrapperPlugin::new(config.into()))
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
      BindingBuiltinPluginName::ViteAlias => {
        let plugin = if let Some(options) = plugin.options {
          BindingViteAliasPluginConfig::from_unknown(options)?.try_into()?
        } else {
          ViteAliasPlugin::default()
        };
        Arc::new(plugin)
      }
      BindingBuiltinPluginName::ViteAsset => {
        let plugin = if let Some(options) = plugin.options {
          BindingViteAssetPluginConfig::from_unknown(options)?.into()
        } else {
          ViteAssetPlugin::default()
        };
        Arc::new(plugin)
      }
      BindingBuiltinPluginName::ViteAssetImportMetaUrl => Arc::new(ViteAssetImportMetaUrlPlugin),
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
      BindingBuiltinPluginName::ViteCSS => {
        let plugin: ViteCSSPlugin = if let Some(options) = plugin.options {
          BindingViteCSSPluginConfig::from_unknown(options)?.into()
        } else {
          return Err(napi::Error::new(
            napi::Status::InvalidArg,
            "Missing options for ViteCSSPlugin",
          ));
        };
        Arc::new(plugin)
      }
      BindingBuiltinPluginName::ViteCSSPost => {
        let plugin = if let Some(options) = plugin.options {
          BindingViteCSSPostPluginConfig::from_unknown(options)?.into()
        } else {
          ViteCSSPostPlugin::default()
        };
        Arc::new(plugin)
      }
      BindingBuiltinPluginName::ViteHtml => {
        let plugin: ViteHtmlPlugin = if let Some(options) = plugin.options {
          BindingViteHtmlPluginConfig::from_unknown(options)?.into()
        } else {
          return Err(napi::Error::new(
            napi::Status::InvalidArg,
            "Missing options for ViteHtmlPlugin",
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
