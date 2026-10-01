use std::{
  path::{Path, PathBuf},
  sync::Arc,
};

use napi::{Task, bindgen_prelude::AsyncTask};
use napi_derive::napi;
use oxc_resolver::Resolver;
use oxc_resolver_napi::ResolverFactory;
use rolldown::EnhancedTransformResult;
use rolldown_common::{
  EnhancedTransformOptions, TsconfigOption, enhanced_transform as core_enhanced_transform,
};
use rolldown_error::BuildDiagnostic;

use crate::options::binding_transform_options::{
  BindingEnhancedTransformOptions, BindingEnhancedTransformResult,
};

fn resolve_tsconfig_from_resolver(
  options: &mut EnhancedTransformOptions,
  resolver: Option<&Resolver>,
  filename: &str,
) -> Result<Option<PathBuf>, BuildDiagnostic> {
  let Some(resolver) = resolver else {
    return Ok(None);
  };
  if matches!(options.tsconfig, Some(TsconfigOption::Config(_) | TsconfigOption::Disabled)) {
    return Ok(None);
  }

  match resolver.find_tsconfig(Path::new(filename)) {
    Ok(Some(tsconfig)) => {
      let path = tsconfig.path.clone();
      options.tsconfig = Some(TsconfigOption::Config(tsconfig));
      Ok(Some(path))
    }
    Ok(None) => {
      options.tsconfig = Some(TsconfigOption::Disabled);
      Ok(None)
    }
    Err(err) => Err(BuildDiagnostic::tsconfig_error(err)),
  }
}

fn enhanced_transform_internal(
  filename: &str,
  source_text: &str,
  options: Option<BindingEnhancedTransformOptions>,
  resolver: Option<&Resolver>,
  yarn_pnp: bool,
) -> napi::Result<BindingEnhancedTransformResult> {
  let options = options.unwrap_or_default();
  let cwd = options
    .cwd
    .clone()
    .map(PathBuf::from)
    .unwrap_or_else(|| std::env::current_dir().expect("Failed to get current dir"));
  let mut transform_options = options.into_enhanced_transform_options(filename)?;
  let tsconfig_path =
    match resolve_tsconfig_from_resolver(&mut transform_options, resolver, filename) {
      Ok(path) => path,
      Err(err) => {
        return Ok(BindingEnhancedTransformResult::from_enhanced_transform_result(
          EnhancedTransformResult::new_for_error(vec![err], vec![], vec![]),
          cwd,
        ));
      }
    };

  let mut result = core_enhanced_transform(filename, source_text, transform_options, yarn_pnp);
  if let Some(path) = tsconfig_path {
    result.tsconfig_file_paths.push(path);
  }
  Ok(BindingEnhancedTransformResult::from_enhanced_transform_result(result, cwd))
}

pub struct EnhancedTransformTask {
  filename: String,
  source_text: String,
  options: Option<BindingEnhancedTransformOptions>,
  resolver: Option<Arc<Resolver>>,
  yarn_pnp: bool,
}

#[napi]
impl Task for EnhancedTransformTask {
  type JsValue = BindingEnhancedTransformResult;
  type Output = BindingEnhancedTransformResult;

  fn compute(&mut self) -> napi::Result<Self::Output> {
    enhanced_transform_internal(
      &self.filename,
      &self.source_text,
      self.options.take(),
      self.resolver.as_deref(),
      self.yarn_pnp,
    )
  }

  fn resolve(&mut self, _env: napi::Env, output: Self::Output) -> napi::Result<Self::JsValue> {
    Ok(output)
  }
}

#[napi]
pub fn enhanced_transform(
  filename: String,
  source_text: String,
  options: Option<BindingEnhancedTransformOptions>,
  resolver: Option<&ResolverFactory>,
  yarn_pnp: bool,
) -> AsyncTask<EnhancedTransformTask> {
  AsyncTask::new(EnhancedTransformTask {
    filename,
    source_text,
    options,
    resolver: resolver.map(ResolverFactory::resolver),
    yarn_pnp,
  })
}

#[napi]
pub fn enhanced_transform_sync(
  filename: String,
  source_text: String,
  options: Option<BindingEnhancedTransformOptions>,
  resolver: Option<&ResolverFactory>,
  yarn_pnp: bool,
) -> napi::Result<BindingEnhancedTransformResult> {
  let resolver = resolver.map(ResolverFactory::resolver);
  enhanced_transform_internal(&filename, &source_text, options, resolver.as_deref(), yarn_pnp)
}
