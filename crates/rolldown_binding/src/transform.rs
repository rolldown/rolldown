use std::path::{Path, PathBuf};

use napi::{Task, bindgen_prelude::AsyncTask};
use napi_derive::napi;
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
