use std::path::PathBuf;
use std::sync::Arc;

use napi::Either;
use napi_derive::napi;
use oxc_napi::get_source_type;
use oxc_sourcemap::napi::SourceMap;
use oxc_transform_napi::{
  CompilerAssumptions, DecoratorOptions, Helpers, JsxOptions, PluginsOptions, TypeScriptOptions,
};
use rolldown_common::{EnhancedTransformOptions, TsconfigOption};
use rustc_hash::FxHashMap;

use crate::types::binding_outputs::to_binding_error;
use crate::types::error::BindingError;
use crate::utils::normalize_binding_transform_options;

fn napi_sourcemap_to_sourcemap(
  map: SourceMap,
) -> Result<rolldown_sourcemap::SourceMap, anyhow::Error> {
  rolldown_sourcemap::SourceMap::from_json(rolldown_sourcemap::JSONSourceMap {
    version: 3,
    file: map.file,
    mappings: map.mappings,
    source_root: map.source_root,
    sources: map.sources,
    sources_content: map.sources_content.map(|v| v.into_iter().map(Some).collect()),
    names: map.names,
    debug_id: None,
    x_google_ignore_list: map.x_google_ignorelist,
  })
  .map_err(|e| anyhow::anyhow!("Failed to convert sourcemap: {e}"))
}

/// TypeScript compiler options for inline tsconfig configuration.
#[napi(object)]
#[derive(Default, Clone)]
pub struct BindingTsconfigCompilerOptions {
  /// Specifies the JSX factory function to use.
  #[napi(ts_type = "'react' | 'react-jsx' | 'react-jsxdev' | 'preserve' | 'react-native'")]
  pub jsx: Option<String>,
  /// Specifies the JSX factory function.
  pub jsx_factory: Option<String>,
  /// Specifies the JSX fragment factory function.
  pub jsx_fragment_factory: Option<String>,
  /// Specifies the module specifier for JSX imports.
  pub jsx_import_source: Option<String>,
  /// Enables experimental decorators.
  pub experimental_decorators: Option<bool>,
  /// Enables decorator metadata emission.
  pub emit_decorator_metadata: Option<bool>,
  /// Preserves module structure of imports/exports.
  pub verbatim_module_syntax: Option<bool>,
  /// Configures how class fields are emitted.
  pub use_define_for_class_fields: Option<bool>,
  /// The ECMAScript target version.
  pub target: Option<String>,
  /// @deprecated Use verbatimModuleSyntax instead.
  pub preserve_value_imports: Option<bool>,
  /// @deprecated Use verbatimModuleSyntax instead.
  #[napi(ts_type = "'remove' | 'preserve' | 'error'")]
  pub imports_not_used_as_values: Option<String>,
}

/// Raw tsconfig options for inline configuration.
#[napi(object)]
#[derive(Default, Clone)]
pub struct BindingTsconfigRawOptions {
  /// TypeScript compiler options.
  pub compiler_options: Option<BindingTsconfigCompilerOptions>,
}

impl From<&BindingTsconfigRawOptions> for oxc_resolver::TsConfig {
  fn from(value: &BindingTsconfigRawOptions) -> Self {
    let mut tsconfig = Self::default();

    if let Some(compiler_options) = &value.compiler_options {
      tsconfig.compiler_options.jsx.clone_from(&compiler_options.jsx);
      tsconfig.compiler_options.jsx_factory.clone_from(&compiler_options.jsx_factory);
      tsconfig
        .compiler_options
        .jsx_fragment_factory
        .clone_from(&compiler_options.jsx_fragment_factory);
      tsconfig.compiler_options.jsx_import_source.clone_from(&compiler_options.jsx_import_source);
      tsconfig.compiler_options.experimental_decorators = compiler_options.experimental_decorators;
      tsconfig.compiler_options.emit_decorator_metadata = compiler_options.emit_decorator_metadata;
      tsconfig.compiler_options.verbatim_module_syntax = compiler_options.verbatim_module_syntax;
      tsconfig.compiler_options.use_define_for_class_fields =
        compiler_options.use_define_for_class_fields;
      tsconfig.compiler_options.target.clone_from(&compiler_options.target);
      tsconfig.compiler_options.preserve_value_imports = compiler_options.preserve_value_imports;
      tsconfig
        .compiler_options
        .imports_not_used_as_values
        .clone_from(&compiler_options.imports_not_used_as_values);
    }

    tsconfig
  }
}

/// Enhanced transform options with tsconfig and inputMap support.
#[napi(object)]
#[derive(Default)]
pub struct BindingEnhancedTransformOptions {
  // --- Oxc transform options ---
  /// Treat the source text as 'js', 'jsx', 'ts', 'tsx', or 'dts'.
  #[napi(ts_type = "'js' | 'jsx' | 'ts' | 'tsx' | 'dts'")]
  pub lang: Option<String>,
  /// Treat the source text as 'script', 'module', 'commonjs', or 'unambiguous'.
  #[napi(ts_type = "'script' | 'module' | 'commonjs' | 'unambiguous' | undefined")]
  pub source_type: Option<String>,
  /// The current working directory. Used to resolve relative paths in other
  /// options.
  pub cwd: Option<String>,
  /// Enable source map generation.
  ///
  /// When `true`, the `sourceMap` field of transform result objects will be populated.
  ///
  /// @default false
  pub sourcemap: Option<bool>,
  /// Set assumptions in order to produce smaller output.
  pub assumptions: Option<CompilerAssumptions>,
  /// Configure how TypeScript is transformed.
  /// @see {@link https://oxc.rs/docs/guide/usage/transformer/typescript}
  pub typescript: Option<TypeScriptOptions>,
  /// Configure how TSX and JSX are transformed.
  /// @see {@link https://oxc.rs/docs/guide/usage/transformer/jsx}
  #[napi(ts_type = "'preserve' | JsxOptions")]
  pub jsx: Option<Either<String, JsxOptions>>,
  /// Sets the target environment for the generated JavaScript.
  ///
  /// The lowest target is `es2015`.
  ///
  /// Example:
  ///
  /// * `'es2015'`
  /// * `['es2020', 'chrome58', 'edge16', 'firefox57', 'node12', 'safari11']`
  ///
  /// @default `esnext` (No transformation)
  ///
  /// @see {@link https://oxc.rs/docs/guide/usage/transformer/lowering#target}
  pub target: Option<Either<String, Vec<String>>>,
  /// Behaviour for runtime helpers.
  pub helpers: Option<Helpers>,
  /// Define Plugin
  /// @see {@link https://oxc.rs/docs/guide/usage/transformer/global-variable-replacement#define}
  #[napi(ts_type = "Record<string, string>")]
  pub define: Option<FxHashMap<String, String>>,
  /// Inject Plugin
  /// @see {@link https://oxc.rs/docs/guide/usage/transformer/global-variable-replacement#inject}
  #[napi(ts_type = "Record<string, string | [string, string]>")]
  pub inject: Option<FxHashMap<String, Either<String, Vec<String>>>>,
  /// Decorator plugin
  pub decorator: Option<DecoratorOptions>,
  /// Third-party plugins to use.
  /// @see {@link https://oxc.rs/docs/guide/usage/transformer/plugins}
  pub plugins: Option<PluginsOptions>,

  // --- Enhanced options ---
  /// Configure tsconfig handling.
  /// - true: Auto-discover and load the nearest tsconfig.json
  /// - TsconfigRawOptions: Use the provided inline tsconfig options
  #[napi(ts_type = "boolean | BindingTsconfigRawOptions")]
  pub tsconfig: Option<Either<bool, BindingTsconfigRawOptions>>,
  /// An input source map to collapse with the output source map.
  pub input_map: Option<SourceMap>,
}

impl BindingEnhancedTransformOptions {
  pub fn into_oxc_options(self) -> oxc_transform_napi::TransformOptions {
    oxc_transform_napi::TransformOptions {
      lang: self.lang,
      source_type: self.source_type,
      cwd: self.cwd,
      sourcemap: self.sourcemap,
      assumptions: self.assumptions,
      typescript: self.typescript,
      jsx: self.jsx,
      target: self.target,
      helpers: self.helpers,
      define: self.define,
      inject: self.inject,
      decorator: self.decorator,
      plugins: self.plugins,
    }
  }
}

/// Result of the enhanced transform API.
#[napi(object, object_from_js = false)]
pub struct BindingEnhancedTransformResult {
  /// The transformed code.
  ///
  /// If parsing failed, this will be an empty string.
  pub code: String,
  /// The source map for the transformed code.
  ///
  /// This will be set if {@link BindingEnhancedTransformOptions#sourcemap} is `true`.
  pub map: Option<SourceMap>,
  /// The `.d.ts` declaration file for the transformed code. Declarations are
  /// only generated if `declaration` is set to `true` and a TypeScript file
  /// is provided.
  ///
  /// If parsing failed and `declaration` is set, this will be an empty string.
  ///
  /// @see {@link TypeScriptOptions#declaration}
  /// @see [declaration tsconfig option](https://www.typescriptlang.org/tsconfig/#declaration)
  pub declaration: Option<String>,
  /// Declaration source map. Only generated if both
  /// {@link TypeScriptOptions#declaration declaration} and
  /// {@link BindingEnhancedTransformOptions#sourcemap sourcemap} are set to `true`.
  pub declaration_map: Option<SourceMap>,
  /// Helpers used.
  ///
  /// @internal
  ///
  /// Example:
  ///
  /// ```text
  /// { "_objectSpread": "@oxc-project/runtime/helpers/objectSpread2" }
  /// ```
  #[napi(ts_type = "Record<string, string>")]
  pub helpers_used: FxHashMap<String, String>,
  /// Parse and transformation errors.
  pub errors: Vec<BindingError>,
  /// Parse and transformation warnings.
  pub warnings: Vec<BindingError>,
  /// Paths to tsconfig files that were loaded during transformation.
  pub tsconfig_file_paths: Vec<String>,
}

impl BindingEnhancedTransformResult {
  pub fn from_enhanced_transform_result(
    result: rolldown_common::EnhancedTransformResult,
    cwd: PathBuf,
  ) -> Self {
    Self {
      code: result.code,
      map: result.map.map(Into::into),
      declaration: result.declaration,
      declaration_map: result.declaration_map.map(Into::into),
      helpers_used: result
        .helpers_used
        .into_iter()
        .map(|(k, v)| (k.name().to_string(), v))
        .collect(),
      errors: result
        .errors
        .into_iter()
        .map(|diagnostic| to_binding_error(&diagnostic, cwd.clone()))
        .collect(),
      warnings: result
        .warnings
        .into_iter()
        .map(|diagnostic| to_binding_error(&diagnostic, cwd.clone()))
        .collect(),
      tsconfig_file_paths: result
        .tsconfig_file_paths
        .into_iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect(),
    }
  }
}

impl BindingEnhancedTransformOptions {
  pub fn into_enhanced_transform_options(
    mut self,
    filename: &str,
  ) -> anyhow::Result<EnhancedTransformOptions> {
    let cwd = self.cwd.clone();
    let source_type =
      Some(get_source_type(filename, self.lang.as_deref(), self.source_type.as_deref()));
    let tsconfig = match &self.tsconfig {
      Some(Either::A(true)) => Some(TsconfigOption::Auto),
      Some(Either::A(false)) => Some(TsconfigOption::Disabled),
      Some(Either::B(raw)) => Some(TsconfigOption::Config(Arc::new(raw.into()))),
      None => None,
    };
    let sourcemap = self.sourcemap.unwrap_or(false);
    let input_map = self.input_map.take().map(napi_sourcemap_to_sourcemap).transpose()?;
    let define = self.define.take().map(|m| m.into_iter().collect::<Vec<_>>());
    let inject = self.inject.take().map(|inject_map| {
      inject_map
        .into_iter()
        .map(|(key, value)| {
          let v = match value {
            Either::A(s) => itertools::Either::Left(s),
            Either::B(v) => itertools::Either::Right(v),
          };
          (key, v)
        })
        .collect()
    });

    let transform_options = normalize_binding_transform_options(self.into_oxc_options());
    Ok(EnhancedTransformOptions::from_transform_options(
      transform_options,
      cwd,
      source_type,
      tsconfig,
      sourcemap,
      input_map,
      define,
      inject,
    ))
  }
}
