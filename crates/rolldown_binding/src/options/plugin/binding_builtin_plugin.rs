use derive_more::Debug;
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
use rolldown_plugin_vite_resolve::{
  FinalizeBareSpecifierCallback, FinalizeOtherSpecifiersCallback, ViteResolveOptions,
  ViteResolvePlugin, ViteResolveResolveOptions,
};
use rolldown_plugin_wasm_fallback::WasmFallbackPlugin;
use rolldown_plugin_wasm_helper::WasmHelperPlugin;
use serde::Deserialize;
use std::{collections::HashMap, sync::Arc};

use super::types::binding_js_or_regex::{bindingify_string_or_regex_array, BindingStringOrRegex};
use super::types::binding_limited_boolean::BindingTrueValue;
use crate::types::js_callback::{JsCallback, JsCallbackExt};

#[allow(clippy::pub_underscore_fields)]
#[napi(object)]
#[derive(Deserialize)]
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
  ViteResolvePlugin,
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

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BindingTransformPluginConfig {
  pub include: Option<Vec<BindingStringOrRegex>>,
  pub exclude: Option<Vec<BindingStringOrRegex>>,
  pub jsx_inject: Option<String>,
  pub targets: Option<String>,
}

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BindingAliasPluginConfig {
  pub entries: Vec<BindingAliasPluginAlias>,
}

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug, Deserialize)]
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

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BindingViteResolvePluginConfig {
  pub resolve_options: BindingViteResolvePluginResolveOptions,
  pub environment_consumer: String,
  pub environment_name: String,
  #[serde(with = "EitherDeserializeEnabler")]
  #[napi(ts_type = "true | string[]")]
  pub external: napi::Either<BindingTrueValue, Vec<String>>,
  #[serde(with = "EitherDeserializeEnabler")]
  #[napi(ts_type = "true | string[]")]
  pub no_external: napi::Either<BindingTrueValue, Vec<String>>,
  #[debug("{}", if finalize_bare_specifier.is_some() { "Some(<finalize_bare_specifier>)" } else { "None" })]
  #[serde(skip_deserializing)]
  #[napi(
    ts_type = "(resolvedId: string, rawId: string, importer: string | null | undefined) => VoidNullable<string>"
  )]
  pub finalize_bare_specifier: Option<JsCallback<(String, String, Option<String>), Option<String>>>,
  #[debug("{}", if finalize_bare_specifier.is_some() { "Some(<finalize_other_specifiers>)" } else { "None" })]
  #[serde(skip_deserializing)]
  #[napi(ts_type = "(resolvedId: string, rawId: string) => VoidNullable<string>")]
  pub finalize_other_specifiers: Option<JsCallback<(String, String), Option<String>>>,

  pub runtime: String,
}

impl From<BindingViteResolvePluginConfig> for ViteResolveOptions {
  fn from(value: BindingViteResolvePluginConfig) -> Self {
    let external = match value.external {
      napi::Either::A(_) => rolldown_plugin_vite_resolve::ResolveOptionsExternal::True,
      napi::Either::B(v) => rolldown_plugin_vite_resolve::ResolveOptionsExternal::Vec(v),
    };
    let no_external = match value.no_external {
      napi::Either::A(_) => rolldown_plugin_vite_resolve::ResolveOptionsNoExternal::True,
      napi::Either::B(v) => rolldown_plugin_vite_resolve::ResolveOptionsNoExternal::Vec(v),
    };

    Self {
      resolve_options: value.resolve_options.into(),
      environment_consumer: value.environment_consumer,
      environment_name: value.environment_name,
      external,
      no_external,
      finalize_bare_specifier: value.finalize_bare_specifier.map(
        |finalizer_fn| -> Arc<FinalizeBareSpecifierCallback> {
          Arc::new(move |resolved_id: &str, raw_id: &str, importer: Option<&str>| {
            let finalizer_fn = Arc::clone(&finalizer_fn);
            let resolved_id = resolved_id.to_owned();
            let raw_id = raw_id.to_owned();
            let importer = importer.map(ToString::to_string);
            Box::pin(async move {
              finalizer_fn
                .invoke_async((resolved_id, raw_id, importer))
                .await
                .map_err(anyhow::Error::from)
            })
          })
        },
      ),
      finalize_other_specifiers: value.finalize_other_specifiers.map(
        |finalizer_fn| -> Arc<FinalizeOtherSpecifiersCallback> {
          Arc::new(move |resolved_id: &str, raw_id: &str| {
            let finalizer_fn = Arc::clone(&finalizer_fn);
            let resolved_id = resolved_id.to_owned();
            let raw_id = raw_id.to_owned();
            Box::pin(async move {
              finalizer_fn.invoke_async((resolved_id, raw_id)).await.map_err(anyhow::Error::from)
            })
          })
        },
      ),

      runtime: value.runtime,
    }
  }
}

#[napi_derive::napi(object)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::struct_excessive_bools)]
pub struct BindingViteResolvePluginResolveOptions {
  pub is_build: bool,
  pub is_production: bool,
  pub as_src: bool,
  pub prefer_relative: bool,
  pub root: String,

  pub main_fields: Vec<String>,
  pub conditions: Vec<String>,
  pub external_conditions: Vec<String>,
  pub extensions: Vec<String>,
  pub try_index: bool,
  pub try_prefix: Option<String>,
  pub preserve_symlinks: bool,
}

impl From<BindingViteResolvePluginResolveOptions> for ViteResolveResolveOptions {
  fn from(value: BindingViteResolvePluginResolveOptions) -> Self {
    Self {
      is_build: value.is_build,
      is_production: value.is_production,
      as_src: value.as_src,
      prefer_relative: value.prefer_relative,
      root: value.root,

      main_fields: value.main_fields,
      conditions: value.conditions,
      external_conditions: value.external_conditions,
      extensions: value.extensions,
      try_index: value.try_index,
      try_prefix: value.try_prefix,
      preserve_symlinks: value.preserve_symlinks,
    }
  }
}

#[derive(Deserialize)]
#[serde(remote = "napi::bindgen_prelude::Either<BindingTrueValue, Vec<String>>")]
enum EitherDeserializeEnabler {
  A(BindingTrueValue),
  B(Vec<String>),
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
      ret.push(Alias { find: item.find.try_into()?, replacement: item.replacement });
    }

    Ok(Self { entries: ret })
  }
}

impl TryFrom<BindingTransformPluginConfig> for TransformPlugin {
  type Error = anyhow::Error;

  fn try_from(value: BindingTransformPluginConfig) -> Result<Self, Self::Error> {
    Ok(TransformPlugin {
      include: value.include.map(bindingify_string_or_regex_array).transpose()?.unwrap_or_default(),
      exclude: value.exclude.map(bindingify_string_or_regex_array).transpose()?.unwrap_or_default(),
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
            delimiters: opts.delimiters.map(|raw| (raw[0].clone(), raw[1].clone())),
            prevent_assignment: opts.prevent_assignment.unwrap_or(false),
            object_guards: opts.object_guards.unwrap_or(false),
            sourcemap: opts.sourcemap.unwrap_or(false),
          }
        })))
      }
      BindingBuiltinPluginName::ViteResolvePlugin => {
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
  pub sourcemap: Option<bool>,
}
