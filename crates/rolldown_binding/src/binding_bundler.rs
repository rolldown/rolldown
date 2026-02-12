// TODO: add reasons about why creating `BindingBundler` instead of reusing `Bundler` of `rolldown` crate.

use crate::{
  classic_bundler::ClassicBundler,
  types::{
    binding_bundler_options::BindingBundlerOptions,
    binding_outputs::{BindingOutputs, to_binding_error},
    error::{BindingError, BindingErrors, BindingResult},
  },
  utils::{handle_result, handle_warnings, normalize_binding_options::normalize_binding_options},
};
use napi::{Env, bindgen_prelude::PromiseRaw};
use napi_derive::napi;
use rolldown::{BundleHandle, BundlerConfig};
use std::sync::Arc;

#[napi]
pub struct BindingBundler {
  inner: ClassicBundler,
  last_bundle_handle: Option<BundleHandle>,
}

#[napi]
impl BindingBundler {
  #[napi(constructor)]
  pub fn new() -> Self {
    let inner = ClassicBundler::new();
    Self { inner, last_bundle_handle: None }
  }

  #[napi]
  pub fn generate<'env>(
    &mut self,
    env: &'env Env,
    options: BindingBundlerOptions<'env>,
  ) -> napi::Result<PromiseRaw<'env, BindingResult<BindingOutputs>>> {
    let normalized = Self::normalize_binding_options(options)?;
    if let Some(result) = Self::validate_hmr_not_allowed(&normalized, "generate") {
      return env.spawn_future(async move { Ok(result) });
    }

    let maybe_bundle = self.inner.create_bundle(normalized.options, normalized.plugins);
    if let Ok(bundle) = &maybe_bundle {
      // Extract bundle handle before consuming the bundle
      self.last_bundle_handle = Some(bundle.context());
    }

    let fut = async move {
      // TODO: we probably advance error handling here instead of waiting for an async call
      let bundle = maybe_bundle.map_err(|err| {
        napi::Error::new(
          napi::Status::GenericFailure,
          err.iter().map(|e| e.to_diagnostic().to_string()).collect::<Vec<_>>().join("\n"),
        )
      })?;
      let cwd = bundle.options().cwd.clone();
      let options = Arc::clone(bundle.options());
      let bundle_output = match bundle.generate().await {
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

      if let Err(err) = handle_warnings(bundle_output.warnings, &options).await {
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
    if let Some(result) = Self::validate_hmr_not_allowed(&normalized, "write") {
      return env.spawn_future(async move { Ok(result) });
    }

    let maybe_bundle = self.inner.create_bundle(normalized.options, normalized.plugins);
    if let Ok(bundle) = &maybe_bundle {
      // Extract bundle handle before consuming the bundle
      self.last_bundle_handle = Some(bundle.context());
    }

    let fut = async move {
      let bundle = maybe_bundle.map_err(|err| {
        napi::Error::new(
          napi::Status::GenericFailure,
          err.iter().map(|e| e.to_diagnostic().to_string()).collect::<Vec<_>>().join("\n"),
        )
      })?;
      let cwd = bundle.options().cwd.clone();
      let options = Arc::clone(bundle.options());
      let bundle_output = match bundle.write().await {
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

      if let Err(err) = handle_warnings(bundle_output.warnings, &options).await {
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
  ) -> napi::Result<PromiseRaw<'env, BindingResult<()>>> {
    let normalized = Self::normalize_binding_options(options)?;
    if let Some(result) = Self::validate_hmr_not_allowed(&normalized, "scan") {
      return env.spawn_future(async move { Ok(result) });
    }

    let maybe_bundle = self.inner.create_bundle(normalized.options, normalized.plugins);
    if let Ok(bundle) = &maybe_bundle {
      // Extract bundle handle before consuming the bundle
      self.last_bundle_handle = Some(bundle.context());
    }

    let fut = async move {
      let bundle = maybe_bundle.map_err(|err| {
        napi::Error::new(
          napi::Status::GenericFailure,
          err.iter().map(|e| e.to_diagnostic().to_string()).collect::<Vec<_>>().join("\n"),
        )
      })?;
      let cwd = bundle.options().cwd.clone();
      match bundle.scan().await {
        Ok(()) => {
          // scan() returns no useful output, just return empty
          Ok(napi::Either::B(()))
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
      .last_bundle_handle
      .as_ref()
      .map(|handle| handle.watch_files().iter().map(|s| s.to_string()).collect())
      .unwrap_or_default()
  }
}

impl BindingBundler {
  fn normalize_binding_options(option: BindingBundlerOptions) -> napi::Result<BundlerConfig> {
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

  /// Validates that dev mode is not enabled for the given API.
  /// Returns an error result if dev mode is enabled.
  fn validate_hmr_not_allowed<T>(
    normalized: &BundlerConfig,
    api_name: &str,
  ) -> Option<BindingResult<T>> {
    if normalized.options.experimental.as_ref().and_then(|e| e.dev_mode.as_ref()).is_some() {
      let message = format!(
        "The \"experimental.devMode\" option is only supported with the \"dev\" API. It cannot be used with \"{api_name}\". Please use the \"dev\" API for dev mode functionality."
      );
      let error = rolldown_error::BuildDiagnostic::bundler_initialize_error(message, None);
      let cwd = normalized.options.cwd.clone().unwrap_or_default();
      let binding_error = to_binding_error(&error, cwd);
      Some(napi::Either::A(BindingErrors::new(vec![binding_error])))
    } else {
      None
    }
  }
}
