// TODO: add reasons about why creating `BindingBundler` instead of reusing `Bundler` of `rolldown` crate.

use crate::{
  binding_bundler_impl::BindingBundlerOptions,
  bundler::Bundler,
  types::{
    binding_outputs::{BindingOutputs, to_binding_error},
    error::{BindingError, BindingErrors, BindingResult},
  },
  utils::{
    handle_result, handle_warnings,
    normalize_binding_options::{NormalizeBindingOptionsReturn, normalize_binding_options},
  },
};
use napi::{Env, bindgen_prelude::PromiseRaw};
use napi_derive::napi;
use rolldown::BuildContext;
use std::sync::Arc;

#[napi]
pub struct BindingBundler {
  inner: Bundler,
  last_build_context: Option<BuildContext>,
}

#[napi]
impl BindingBundler {
  #[napi(constructor)]
  pub fn new() -> napi::Result<Self> {
    let inner = Bundler::new();
    Ok(Self { inner, last_build_context: None })
  }

  #[napi]
  pub fn generate<'env>(
    &mut self,
    env: &'env Env,
    options: BindingBundlerOptions<'env>,
  ) -> napi::Result<PromiseRaw<'env, BindingResult<BindingOutputs>>> {
    let normalized = Self::normalize_binding_options(options)?;
    let maybe_build = self.inner.create_build(normalized.bundler_options, normalized.plugins);
    if let Ok((build, _)) = &maybe_build {
      // Extract build context before consuming the build
      self.last_build_context = Some(build.context());
    }

    let fut = async move {
      // TODO: we probably advance error handling here instead of waiting for an async call
      let (build, mut warnings_for_creating_build) = maybe_build.map_err(|err| {
        napi::Error::new(
          napi::Status::GenericFailure,
          err.iter().map(|e| e.to_diagnostic().to_string()).collect::<Vec<_>>().join("\n"),
        )
      })?;
      let cwd = build.options().cwd.clone();
      let options = Arc::clone(build.options());
      let bundle_output = match build.generate().await {
        Ok(output) => output,
        Err(errs) => {
          let errors: Vec<BindingError> = errs
            .into_vec()
            .iter()
            .map(|diagnostic| to_binding_error(diagnostic, cwd.clone()))
            .collect();
          return Ok(napi::Either::A(BindingErrors::new(errors)));
        }
      };

      warnings_for_creating_build.extend(bundle_output.warnings);

      if let Err(err) = handle_warnings(warnings_for_creating_build, &options).await {
        let error = to_binding_error(&err.into(), cwd.clone());
        return Ok(napi::Either::A(BindingErrors::new(vec![error])));
      }

      Ok(napi::Either::B(bundle_output.assets.into()))
    };
    env.spawn_future(fut)
  }

  #[napi]
  pub fn write<'env>(
    &mut self,
    env: &'env Env,
    options: BindingBundlerOptions<'env>,
  ) -> napi::Result<PromiseRaw<'env, BindingResult<BindingOutputs>>> {
    let normalized = Self::normalize_binding_options(options)?;
    let maybe_build = self.inner.create_build(normalized.bundler_options, normalized.plugins);
    if let Ok((build, _)) = &maybe_build {
      // Extract build context before consuming the build
      self.last_build_context = Some(build.context());
    }

    let fut = async move {
      let (build, mut warnings_for_creating_build) = maybe_build.map_err(|err| {
        napi::Error::new(
          napi::Status::GenericFailure,
          err.iter().map(|e| e.to_diagnostic().to_string()).collect::<Vec<_>>().join("\n"),
        )
      })?;
      let cwd = build.options().cwd.clone();
      let options = Arc::clone(build.options());
      let bundle_output = match build.write().await {
        Ok(output) => output,
        Err(errs) => {
          let errors: Vec<BindingError> = errs
            .into_vec()
            .iter()
            .map(|diagnostic| to_binding_error(diagnostic, cwd.clone()))
            .collect();
          return Ok(napi::Either::A(BindingErrors::new(errors)));
        }
      };

      warnings_for_creating_build.extend(bundle_output.warnings);

      if let Err(err) = handle_warnings(warnings_for_creating_build, &options).await {
        let error = to_binding_error(&err.into(), cwd.clone());
        return Ok(napi::Either::A(BindingErrors::new(vec![error])));
      }

      Ok(napi::Either::B(bundle_output.assets.into()))
    };
    env.spawn_future(fut)
  }

  #[napi]
  pub fn scan<'env>(
    &mut self,
    env: &'env Env,
    options: BindingBundlerOptions<'env>,
  ) -> napi::Result<PromiseRaw<'env, BindingResult<BindingOutputs>>> {
    let normalized = Self::normalize_binding_options(options)?;
    let maybe_build = self.inner.create_build(normalized.bundler_options, normalized.plugins);
    if let Ok((build, _)) = &maybe_build {
      // Extract build context before consuming the build
      self.last_build_context = Some(build.context());
    }

    let fut = async move {
      let (build, _warnings_for_creating_build) = maybe_build.map_err(|err| {
        napi::Error::new(
          napi::Status::GenericFailure,
          err.iter().map(|e| e.to_diagnostic().to_string()).collect::<Vec<_>>().join("\n"),
        )
      })?;
      let cwd = build.options().cwd.clone();
      match build.scan().await {
        Ok(()) => {
          // scan() returns no useful output, just return empty
          Ok(napi::Either::B(vec![].into()))
        }
        Err(errs) => {
          let errors: Vec<BindingError> = errs
            .into_vec()
            .iter()
            .map(|diagnostic| to_binding_error(diagnostic, cwd.clone()))
            .collect();
          Ok(napi::Either::A(BindingErrors::new(errors)))
        }
      }
    };
    env.spawn_future(fut)
  }

  #[napi]
  // - `Bundler::close()/inner.close()` requires acquiring `&mut self`
  // - Acquiring `&mut self` in async napi `fn` is unsafe, so we must use a sync `fn` here.
  // - But `Bundler::close()/inner.close()` contains async cleanup operations, so we have await its returned future
  // in another async context instead of directly calling `close().await`.
  // - This also affects how the code is written in `Bundler::close()/inner.close()`, see the implementation there for more details.
  pub fn close<'env>(&mut self, env: &'env Env) -> napi::Result<PromiseRaw<'env, ()>> {
    let cleanup_fut = self.inner.close();
    env.spawn_future(async move {
      let res = cleanup_fut.await;
      handle_result(res)?;
      Ok(())
    })
  }

  #[napi(getter)]
  pub fn closed(&self) -> bool {
    self.inner.closed()
  }

  #[napi]
  pub fn get_watch_files(&self) -> Vec<String> {
    self
      .last_build_context
      .as_ref()
      .map(|context| context.watch_files().iter().map(|s| s.to_string()).collect())
      .unwrap_or_default()
  }
}

impl BindingBundler {
  fn normalize_binding_options(
    option: BindingBundlerOptions,
  ) -> napi::Result<NormalizeBindingOptionsReturn> {
    let BindingBundlerOptions { input_options, output_options, parallel_plugins_registry } = option;

    #[cfg(not(target_family = "wasm"))]
    let worker_count =
      parallel_plugins_registry.as_ref().map(|registry| registry.worker_count).unwrap_or_default();
    #[cfg(not(target_family = "wasm"))]
    let parallel_plugins_map =
      parallel_plugins_registry.map(|registry| registry.take_plugin_values());

    #[cfg(not(target_family = "wasm"))]
    let worker_manager = if worker_count > 0 {
      use crate::worker_manager::WorkerManager;
      Some(WorkerManager::new(worker_count))
    } else {
      None
    };

    let ret = normalize_binding_options(
      input_options,
      output_options,
      #[cfg(not(target_family = "wasm"))]
      parallel_plugins_map,
      #[cfg(not(target_family = "wasm"))]
      worker_manager,
    )?;

    Ok(ret)
  }
}
