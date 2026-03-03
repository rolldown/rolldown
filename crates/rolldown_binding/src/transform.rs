use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use napi::{Task, bindgen_prelude::AsyncTask};
use napi_derive::napi;
use oxc_resolver::{CompilerOptions, TsConfig};
use rolldown::EnhancedTransformResult;
use rolldown_common::{
  EnhancedTransformOptions, TsconfigOption, enhanced_transform as core_enhanced_transform,
};
use rolldown_error::BuildDiagnostic;

use crate::options::binding_transform_options::{
  BindingEnhancedTransformOptions, BindingEnhancedTransformResult,
};
use crate::transform_cache::TsconfigCache;

fn resolve_tsconfig_from_cache(
  options: &mut EnhancedTransformOptions,
  cache: Option<&TsconfigCache>,
  filename: &str,
) -> Result<(), BuildDiagnostic> {
  let Some(tsconfig_cache) = cache else {
    return Ok(());
  };
  if !matches!(options.tsconfig, Some(TsconfigOption::Auto) | None) {
    return Ok(());
  }

  match tsconfig_cache.find_tsconfig(Path::new(filename)) {
    Ok(Some(tsconfig)) => {
      options.tsconfig = Some(TsconfigOption::Config(tsconfig));
    }
    Ok(None) => {}
    Err(err) => {
      return Err(BuildDiagnostic::tsconfig_error(filename.to_string(), err));
    }
  }
  Ok(())
}

fn enhanced_transform_internal(
  filename: &str,
  source_text: &str,
  options: Option<BindingEnhancedTransformOptions>,
  cache: Option<&TsconfigCache>,
) -> napi::Result<BindingEnhancedTransformResult> {
  let options = options.unwrap_or_default();
  let cwd = options
    .cwd
    .clone()
    .map(PathBuf::from)
    .unwrap_or_else(|| std::env::current_dir().expect("Failed to get current dir"));
  let mut transform_options = options.into_enhanced_transform_options(filename)?;
  if let Err(err) = resolve_tsconfig_from_cache(&mut transform_options, cache, filename) {
    return Ok(BindingEnhancedTransformResult::from_enhanced_transform_result(
      EnhancedTransformResult::new_for_error(vec![err], vec![], vec![]),
      cwd,
    ));
  }

  let result = core_enhanced_transform(filename, source_text, transform_options);
  Ok(BindingEnhancedTransformResult::from_enhanced_transform_result(result, cwd))
}

pub struct EnhancedTransformTask<'a> {
  filename: String,
  source_text: String,
  options: Option<BindingEnhancedTransformOptions>,
  cache: Option<&'a TsconfigCache>,
}

#[napi]
impl Task for EnhancedTransformTask<'_> {
  type JsValue = BindingEnhancedTransformResult;
  type Output = BindingEnhancedTransformResult;

  fn compute(&mut self) -> napi::Result<Self::Output> {
    enhanced_transform_internal(&self.filename, &self.source_text, self.options.take(), self.cache)
  }

  fn resolve(&mut self, _env: napi::Env, output: Self::Output) -> napi::Result<Self::JsValue> {
    Ok(output)
  }
}

/// Transpile a JavaScript or TypeScript into a target ECMAScript version, asynchronously.
///
/// Note: This function can be slower than `transformSync` due to the overhead of spawning a thread.
///
/// @param filename The name of the file being transformed. If this is a
/// relative path, consider setting the {@link TransformOptions#cwd} option.
/// @param sourceText The source code to transform.
/// @param options The transform options including tsconfig and inputMap. See {@link
/// BindingEnhancedTransformOptions} for more information.
/// @param cache Optional tsconfig cache for reusing resolved tsconfig across multiple transforms.
/// Only used when tsconfig auto-discovery is enabled.
///
/// @returns a promise that resolves to an object containing the transformed code,
/// source maps, and any errors that occurred during parsing or transformation.
///
/// @experimental
#[napi]
pub fn enhanced_transform(
  filename: String,
  source_text: String,
  options: Option<BindingEnhancedTransformOptions>,
  cache: Option<&TsconfigCache>,
) -> AsyncTask<EnhancedTransformTask<'_>> {
  AsyncTask::new(EnhancedTransformTask { filename, source_text, options, cache })
}

/// Transpile a JavaScript or TypeScript into a target ECMAScript version.
///
/// @param filename The name of the file being transformed. If this is a
/// relative path, consider setting the {@link TransformOptions#cwd} option.
/// @param sourceText The source code to transform.
/// @param options The transform options including tsconfig and inputMap. See {@link
/// BindingEnhancedTransformOptions} for more information.
/// @param cache Optional tsconfig cache for reusing resolved tsconfig across multiple transforms.
/// Only used when tsconfig auto-discovery is enabled.
///
/// @returns an object containing the transformed code, source maps, and any errors
/// that occurred during parsing or transformation.
///
/// @experimental
#[napi]
pub fn enhanced_transform_sync(
  filename: String,
  source_text: String,
  options: Option<BindingEnhancedTransformOptions>,
  cache: Option<&TsconfigCache>,
) -> napi::Result<BindingEnhancedTransformResult> {
  enhanced_transform_internal(&filename, &source_text, options, cache)
}

/// @hidden This is only expected to be used by Vite
#[napi]
pub fn resolve_tsconfig(
  filename: String,
  cache: Option<&TsconfigCache>,
) -> napi::Result<Option<BindingTsconfigResult>> {
  let tsconfig_cache = if let Some(cache) = cache { cache } else { &TsconfigCache::new() };

  match tsconfig_cache.find_tsconfig(Path::new(&filename)) {
    Ok(Some(tsconfig)) => Ok(Some(tsconfig.as_ref().clone().into())),
    Ok(None) => Ok(None),
    Err(err) => {
      Err(napi::Error::from_reason(format!("Failed to resolve tsconfig for {filename}: {err}")))
    }
  }
}

fn pathbufs_into_strings(paths: Option<Vec<PathBuf>>) -> Option<Vec<String>> {
  paths.map(|vec| vec.into_iter().map(|path_buf| path_buf.to_string_lossy().to_string()).collect())
}

#[napi(object, object_from_js = false)]
pub struct BindingTsconfigResult {
  pub tsconfig: BindingTsconfig,
  pub tsconfig_file_paths: Vec<String>,
}

impl From<TsConfig> for BindingTsconfigResult {
  fn from(tsconfig: TsConfig) -> Self {
    let tsconfig_file_paths = vec![tsconfig.path.to_string_lossy().to_string()];
    Self { tsconfig: tsconfig.into(), tsconfig_file_paths }
  }
}

#[napi(object, object_from_js = false)]
pub struct BindingTsconfig {
  pub files: Option<Vec<String>>,
  pub include: Option<Vec<String>>,
  pub exclude: Option<Vec<String>>,
  pub compiler_options: BindingCompilerOptions,
}

impl From<TsConfig> for BindingTsconfig {
  fn from(tsconfig: TsConfig) -> Self {
    Self {
      files: pathbufs_into_strings(tsconfig.files),
      include: pathbufs_into_strings(tsconfig.include),
      exclude: pathbufs_into_strings(tsconfig.exclude),
      compiler_options: tsconfig.compiler_options.into(),
    }
  }
}

#[napi(object, object_from_js = false)]
pub struct BindingCompilerOptions {
  pub base_url: Option<String>,
  pub paths: Option<IndexMap<String, Vec<String>>>,
  pub experimental_decorators: Option<bool>,
  pub emit_decorator_metadata: Option<bool>,
  pub use_define_for_class_fields: Option<bool>,
  pub rewrite_relative_import_extensions: Option<bool>,
  pub jsx: Option<String>,
  pub jsx_factory: Option<String>,
  pub jsx_fragment_factory: Option<String>,
  pub jsx_import_source: Option<String>,
  pub verbatim_module_syntax: Option<bool>,
  pub preserve_value_imports: Option<bool>,
  pub imports_not_used_as_values: Option<String>,
  pub target: Option<String>,
  pub module: Option<String>,
  pub allow_js: Option<bool>,
  pub root_dirs: Option<Vec<String>>,
}

impl From<CompilerOptions> for BindingCompilerOptions {
  fn from(options: CompilerOptions) -> Self {
    Self {
      base_url: options.base_url.map(|p| p.to_string_lossy().to_string()),
      paths: options.paths.map(|p| {
        p.into_iter()
          .map(|(k, v)| {
            (k, v.into_iter().map(|path_buf| path_buf.to_string_lossy().to_string()).collect())
          })
          .collect()
      }),
      experimental_decorators: options.experimental_decorators,
      emit_decorator_metadata: options.emit_decorator_metadata,
      use_define_for_class_fields: options.use_define_for_class_fields,
      rewrite_relative_import_extensions: options.rewrite_relative_import_extensions,
      jsx: options.jsx,
      jsx_factory: options.jsx_factory,
      jsx_fragment_factory: options.jsx_fragment_factory,
      jsx_import_source: options.jsx_import_source,
      verbatim_module_syntax: options.verbatim_module_syntax,
      preserve_value_imports: options.preserve_value_imports,
      imports_not_used_as_values: options.imports_not_used_as_values,
      target: options.target,
      module: options.module,
      allow_js: options.allow_js,
      root_dirs: pathbufs_into_strings(options.root_dirs),
    }
  }
}
