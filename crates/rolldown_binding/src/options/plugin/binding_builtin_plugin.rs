use derive_more::Debug;
use napi::Either;
use napi::JsUnknown;
use napi::bindgen_prelude::FnArgs;
use napi::bindgen_prelude::FromNapiValue;
use napi_derive::napi;
use oxc_transform_napi::TransformOptions;
use rolldown_plugin::__inner::Pluginable;
use rolldown_plugin_alias::{Alias, AliasPlugin};
use rolldown_plugin_build_import_analysis::BuildImportAnalysisPlugin;
use rolldown_plugin_dynamic_import_vars::DynamicImportVarsPlugin;
use rolldown_plugin_import_glob::{ImportGlobPlugin, ImportGlobPluginConfig};
use rolldown_plugin_isolated_declaration::IsolatedDeclarationPlugin;
use rolldown_plugin_json::{JsonPlugin, JsonPluginStringify};
use rolldown_plugin_load_fallback::LoadFallbackPlugin;
use rolldown_plugin_manifest::{ManifestPlugin, ManifestPluginConfig};
use rolldown_plugin_module_federation::ModuleFederationPlugin;
use rolldown_plugin_module_preload_polyfill::ModulePreloadPolyfillPlugin;
use rolldown_plugin_replace::{ReplaceOptions, ReplacePlugin};
use rolldown_plugin_report::ReportPlugin;
use rolldown_plugin_transform::TransformPlugin;
use rolldown_plugin_vite_resolve::{
  FinalizeBareSpecifierCallback, FinalizeOtherSpecifiersCallback, ViteResolveOptions,
  ViteResolvePlugin, ViteResolveResolveOptions,
};
use rolldown_plugin_wasm_fallback::WasmFallbackPlugin;
use rolldown_plugin_wasm_helper::WasmHelperPlugin;
use rustc_hash::{FxBuildHasher, FxHashSet};
use std::collections::HashMap;
use std::sync::Arc;

use super::types::binding_builtin_plugin_name::BindingBuiltinPluginName;
use super::types::binding_js_or_regex::{BindingStringOrRegex, bindingify_string_or_regex_array};
use super::types::binding_limited_boolean::BindingTrueValue;
use super::types::binding_module_federation_plugin_option::BindingModuleFederationPluginOption;
use crate::types::js_callback::{JsCallback, JsCallbackExt};

#[allow(clippy::pub_underscore_fields)]
#[napi(object)]
pub struct BindingBuiltinPlugin {
  #[napi(js_name = "__name")]
  pub __name: BindingBuiltinPluginName,
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

#[napi_derive::napi(object)]
#[derive(Debug, Default)]
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
#[derive(Debug, Default)]
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
#[derive(Debug, Default)]
pub struct BindingModulePreloadPolyfillPluginConfig {
  pub skip: Option<bool>,
}

#[napi_derive::napi(object)]
#[derive(Debug, Default)]
pub struct BindingJsonPluginConfig {
  pub minify: Option<bool>,
  pub named_exports: Option<bool>,
  pub stringify: Option<BindingJsonPluginStringify>,
}

#[derive(Debug)]
#[napi(transparent)]
pub struct BindingJsonPluginStringify(napi::Either<bool, String>);

impl TryFrom<BindingJsonPluginStringify> for JsonPluginStringify {
  type Error = napi::Error;

  fn try_from(value: BindingJsonPluginStringify) -> Result<Self, Self::Error> {
    Ok(match value {
      BindingJsonPluginStringify(napi::Either::A(true)) => JsonPluginStringify::True,
      BindingJsonPluginStringify(napi::Either::A(false)) => JsonPluginStringify::False,
      BindingJsonPluginStringify(napi::Either::B(s)) if s == "auto" => JsonPluginStringify::Auto,
      BindingJsonPluginStringify(napi::Either::B(s)) => {
        return Err(napi::Error::new(
          napi::Status::InvalidArg,
          format!("Invalid stringify option: {s}"),
        ));
      }
    })
  }
}

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Default)]
pub struct BindingTransformPluginConfig {
  pub include: Option<Vec<BindingStringOrRegex>>,
  pub exclude: Option<Vec<BindingStringOrRegex>>,
  pub jsx_refresh_include: Option<Vec<BindingStringOrRegex>>,
  pub jsx_refresh_exclude: Option<Vec<BindingStringOrRegex>>,

  pub is_server_consumer: Option<bool>,
  pub runtime_resolve_base: Option<String>,

  pub jsx_inject: Option<String>,
  pub transform_options: Option<TransformOptions>,
}

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug, Default)]
pub struct BindingAliasPluginConfig {
  pub entries: Vec<BindingAliasPluginAlias>,
}

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug)]
pub struct BindingAliasPluginAlias {
  pub find: BindingStringOrRegex,
  pub replacement: String,
}

#[napi_derive::napi(object)]
#[derive(Debug, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct BindingBuildImportAnalysisPluginConfig {
  pub preload_code: String,
  pub insert_preload: bool,
  pub optimize_module_preload_relative_paths: bool,
  pub render_built_url: bool,
  pub is_relative_base: bool,
}

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug)]
pub struct BindingViteResolvePluginConfig {
  pub resolve_options: BindingViteResolvePluginResolveOptions,
  pub environment_consumer: String,
  pub environment_name: String,
  #[napi(ts_type = "true | string[]")]
  pub external: napi::Either<BindingTrueValue, Vec<String>>,
  #[napi(ts_type = "true | Array<string | RegExp>")]
  pub no_external: napi::Either<BindingTrueValue, Vec<BindingStringOrRegex>>,
  pub dedupe: Vec<String>,
  #[debug("{}", if finalize_bare_specifier.is_some() { "Some(<finalize_bare_specifier>)" } else { "None" })]
  #[napi(
    ts_type = "(resolvedId: string, rawId: string, importer: string | null | undefined) => VoidNullable<string>"
  )]
  pub finalize_bare_specifier:
    Option<JsCallback<FnArgs<(String, String, Option<String>)>, Option<String>>>,
  #[debug("{}", if finalize_bare_specifier.is_some() { "Some(<finalize_other_specifiers>)" } else { "None" })]
  #[napi(ts_type = "(resolvedId: string, rawId: string) => VoidNullable<string>")]
  pub finalize_other_specifiers: Option<JsCallback<FnArgs<(String, String)>, Option<String>>>,

  pub runtime: String,
}

impl From<BindingViteResolvePluginConfig> for ViteResolveOptions {
  fn from(value: BindingViteResolvePluginConfig) -> Self {
    let external = match value.external {
      napi::Either::A(_) => rolldown_plugin_vite_resolve::ResolveOptionsExternal::True,
      napi::Either::B(v) => rolldown_plugin_vite_resolve::ResolveOptionsExternal::Vec(v),
    };
    let no_external = match value.no_external {
      napi::Either::A(_) => rolldown_plugin_vite_resolve::ResolveOptionsNoExternal::new_true(),
      napi::Either::B(v) => rolldown_plugin_vite_resolve::ResolveOptionsNoExternal::new_vec(
        bindingify_string_or_regex_array(v),
      ),
    };

    Self {
      resolve_options: value.resolve_options.into(),
      environment_consumer: value.environment_consumer,
      environment_name: value.environment_name,
      external,
      no_external,
      dedupe: value.dedupe,
      finalize_bare_specifier: value.finalize_bare_specifier.map(
        |finalizer_fn| -> Arc<FinalizeBareSpecifierCallback> {
          Arc::new(move |resolved_id: &str, raw_id: &str, importer: Option<&str>| {
            let finalizer_fn = Arc::clone(&finalizer_fn);
            let resolved_id = resolved_id.to_owned();
            let raw_id = raw_id.to_owned();
            let importer = importer.map(ToString::to_string);
            Box::pin(async move {
              finalizer_fn
                .invoke_async((resolved_id, raw_id, importer).into())
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
              finalizer_fn
                .invoke_async((resolved_id, raw_id).into())
                .await
                .map_err(anyhow::Error::from)
            })
          })
        },
      ),

      runtime: value.runtime,
    }
  }
}

#[napi_derive::napi(object)]
#[derive(Debug)]
#[allow(clippy::struct_excessive_bools)]
pub struct BindingViteResolvePluginResolveOptions {
  pub is_build: bool,
  pub is_production: bool,
  pub as_src: bool,
  pub prefer_relative: bool,
  pub is_require: Option<bool>,
  pub root: String,
  pub scan: bool,

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
      is_require: value.is_require,
      root: value.root,
      scan: value.scan,

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
      ret.push(Alias { find: item.find.into(), replacement: item.replacement });
    }

    Ok(Self { entries: ret })
  }
}

impl From<BindingTransformPluginConfig> for TransformPlugin {
  fn from(value: BindingTransformPluginConfig) -> Self {
    let sourcemap = value.transform_options.as_ref().and_then(|v| v.sourcemap).unwrap_or(true);
    let transform_options = value.transform_options.map(|v| {
      let jsx = v.jsx.map(|jsx| match jsx {
        Either::A(jsx) => itertools::Either::Left(jsx),
        Either::B(jsx) => {
          let refresh = jsx.refresh.map(|refresh| match refresh {
            Either::A(refresh) => itertools::Either::Left(refresh),
            Either::B(refresh) => {
              itertools::Either::Right(rolldown_plugin_transform::ReactRefreshOptions {
                refresh_reg: refresh.refresh_reg,
                refresh_sig: refresh.refresh_sig,
                emit_full_signatures: refresh.emit_full_signatures,
              })
            }
          });
          itertools::Either::Right(rolldown_plugin_transform::JsxOptions {
            runtime: jsx.runtime,
            development: jsx.development,
            throw_if_namespace: jsx.throw_if_namespace,
            pure: jsx.pure,
            import_source: jsx.import_source,
            pragma: jsx.pragma,
            pragma_frag: jsx.pragma_frag,
            use_built_ins: jsx.use_built_ins,
            use_spread: jsx.use_spread,
            refresh,
          })
        }
      });

      let target = v.target.map(|target| match target {
        Either::A(v) => itertools::Either::Left(v),
        Either::B(v) => itertools::Either::Right(v),
      });

      let decorator = v.decorator.map(|decorator| rolldown_plugin_transform::DecoratorOptions {
        legacy: decorator.legacy,
        emit_decorator_metadata: decorator.emit_decorator_metadata,
      });

      let typescript = v.typescript.map(|typescript| {
        let declaration = typescript.declaration.map(|declaration| {
          rolldown_plugin_transform::IsolatedDeclarationsOptions {
            strip_internal: declaration.strip_internal,
            sourcemap: declaration.sourcemap,
          }
        });
        let rewrite_import_extensions = typescript.rewrite_import_extensions.map(|v| match v {
          Either::A(v) => itertools::Either::Left(v),
          Either::B(v) => itertools::Either::Right(v),
        });

        rolldown_plugin_transform::TypeScriptOptions {
          declaration,
          jsx_pragma: typescript.jsx_pragma,
          jsx_pragma_frag: typescript.jsx_pragma_frag,
          only_remove_type_imports: typescript.only_remove_type_imports,
          allow_namespaces: typescript.allow_namespaces,
          allow_declare_fields: typescript.allow_declare_fields,
          rewrite_import_extensions,
        }
      });

      let assumptions = v.assumptions.map(|v| rolldown_plugin_transform::CompilerAssumptions {
        ignore_function_length: v.ignore_function_length,
        no_document_all: v.no_document_all,
        object_rest_no_symbols: v.object_rest_no_symbols,
        pure_getters: v.pure_getters,
        set_public_class_fields: v.set_public_class_fields,
      });

      rolldown_plugin_transform::TransformOptions {
        lang: v.lang,
        jsx,
        target,
        decorator,
        typescript,
        assumptions,
      }
    });

    Self {
      include: value.include.map(bindingify_string_or_regex_array).unwrap_or_default(),
      exclude: value.exclude.map(bindingify_string_or_regex_array).unwrap_or_default(),
      jsx_refresh_include: value
        .jsx_refresh_include
        .map(bindingify_string_or_regex_array)
        .unwrap_or_default(),
      jsx_refresh_exclude: value
        .jsx_refresh_exclude
        .map(bindingify_string_or_regex_array)
        .unwrap_or_default(),

      jsx_inject: value.jsx_inject,

      is_server_consumer: value.is_server_consumer.unwrap_or(true),
      runtime_resolve_base: value.runtime_resolve_base,

      sourcemap,
      transform_options: transform_options.unwrap_or_default(),
    }
  }
}

impl TryFrom<BindingBuiltinPlugin> for Arc<dyn Pluginable> {
  type Error = napi::Error;

  #[allow(clippy::too_many_lines)]
  fn try_from(plugin: BindingBuiltinPlugin) -> Result<Self, Self::Error> {
    Ok(match plugin.__name {
      BindingBuiltinPluginName::WasmHelper => Arc::new(WasmHelperPlugin {}),
      BindingBuiltinPluginName::WasmFallback => Arc::new(WasmFallbackPlugin {}),
      BindingBuiltinPluginName::ImportGlob => {
        let config = if let Some(options) = plugin.options {
          BindingGlobImportPluginConfig::from_unknown(options)?.into()
        } else {
          ImportGlobPluginConfig::default()
        };
        Arc::new(ImportGlobPlugin { config })
      }
      BindingBuiltinPluginName::DynamicImportVars => Arc::new(DynamicImportVarsPlugin {}),
      BindingBuiltinPluginName::ModulePreloadPolyfill => Arc::new(ModulePreloadPolyfillPlugin {}),
      BindingBuiltinPluginName::Manifest => {
        let config = if let Some(options) = plugin.options {
          BindingManifestPluginConfig::from_unknown(options)?.into()
        } else {
          ManifestPluginConfig::default()
        };
        Arc::new(ManifestPlugin { config, entry_css_asset_file_names: FxHashSet::default() })
      }
      BindingBuiltinPluginName::LoadFallback => Arc::new(LoadFallbackPlugin {}),
      BindingBuiltinPluginName::Transform => {
        let plugin = if let Some(options) = plugin.options {
          BindingTransformPluginConfig::from_unknown(options)?.into()
        } else {
          TransformPlugin::default()
        };
        Arc::new(plugin)
      }
      BindingBuiltinPluginName::Alias => {
        let plugin = if let Some(options) = plugin.options {
          BindingAliasPluginConfig::from_unknown(options)?.try_into()?
        } else {
          AliasPlugin::default()
        };
        Arc::new(plugin)
      }
      BindingBuiltinPluginName::Json => {
        let config = if let Some(options) = plugin.options {
          BindingJsonPluginConfig::from_unknown(options)?
        } else {
          BindingJsonPluginConfig::default()
        };
        Arc::new(JsonPlugin {
          minify: config.minify.unwrap_or_default(),
          named_exports: config.named_exports.unwrap_or_default(),
          stringify: config.stringify.map(TryInto::try_into).transpose()?.unwrap_or_default(),
        })
      }
      BindingBuiltinPluginName::BuildImportAnalysis => {
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
      BindingBuiltinPluginName::Replace => {
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
      BindingBuiltinPluginName::IsolatedDeclaration => {
        let plugin = if let Some(options) = plugin.options {
          BindingIsolatedDeclarationPluginConfig::from_unknown(options)?.into()
        } else {
          IsolatedDeclarationPlugin::default()
        };
        Arc::new(plugin)
      }
      BindingBuiltinPluginName::Report => {
        let plugin: ReportPlugin = if let Some(options) = plugin.options {
          BindingReportPluginConfig::from_unknown(options)?.into()
        } else {
          return Err(napi::Error::new(
            napi::Status::InvalidArg,
            "Missing options for ReportPlugin",
          ));
        };
        Arc::new(plugin)
      }
    })
  }
}

#[napi_derive::napi(object)]
#[derive(Debug, Default)]
pub struct BindingIsolatedDeclarationPluginConfig {
  pub strip_internal: Option<bool>,
}

impl From<BindingIsolatedDeclarationPluginConfig> for IsolatedDeclarationPlugin {
  fn from(value: BindingIsolatedDeclarationPluginConfig) -> Self {
    IsolatedDeclarationPlugin { strip_internal: value.strip_internal.unwrap_or_default() }
  }
}

#[napi_derive::napi(object)]
#[derive(Debug, Default)]
pub struct BindingReplacePluginConfig {
  // It's ok we use `HashMap` here, because we don't care about the order of the keys.
  pub values: HashMap<String, String, FxBuildHasher>,
  #[napi(ts_type = "[string, string]")]
  pub delimiters: Option<Vec<String>>,
  pub prevent_assignment: Option<bool>,
  pub object_guards: Option<bool>,
  pub sourcemap: Option<bool>,
}

#[napi_derive::napi(object)]
#[derive(Debug, Default)]
pub struct BindingReportPluginConfig {
  pub is_tty: bool,
}

impl From<BindingReportPluginConfig> for ReportPlugin {
  fn from(config: BindingReportPluginConfig) -> Self {
    ReportPlugin::new(config.is_tty)
  }
}
