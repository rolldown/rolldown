use itertools::Itertools;
use napi_derive::napi;
use rolldown_dev::{
  BundleState, OnAdditionalAssetsCallback, OnHmrUpdatesCallback, OnOutputCallback,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::binding_dev_options::BindingDevOptions;
use crate::types::binding_bundler_options::BindingBundlerOptions;
use crate::types::binding_error_stage::BindingErrorStage;
use crate::types::binding_hmr_update::BindingHmrUpdate;
use crate::types::binding_module_info::BindingModuleInfo;
use crate::types::binding_outputs::{BindingOutputs, to_binding_error};
use crate::types::error::{BindingErrors, BindingResult};
use crate::types::js_callback_release::{RequestReleaseOnDrop, SharedJsCallbackRelease};
use crate::utils::{
  create_bundler_config_from_binding_options::create_bundler_config_from_binding_options,
  spawn_boxed_future,
};
use napi::bindgen_prelude::{FnArgs, PromiseRaw};
use napi::{Either, Env, threadsafe_function::ThreadsafeFunctionCallMode};
use rolldown_common::HmrPullResponse;

#[napi]
pub struct BindingDevEngine {
  inner: Arc<rolldown_dev::DevEngine>,
  cwd: Arc<Path>,
  _session_id: Arc<str>,
  _session: rolldown_devtools::Session,
  /// The JS functions of the options. `close` requests their release.
  /// See internal-docs/bundler-data-lifecycle/implementation.md ("JS callback lifetime").
  option_callbacks: SharedJsCallbackRelease,
}

#[napi]
impl BindingDevEngine {
  #[napi(constructor)]
  pub fn new(
    env: Env,
    options: BindingBundlerOptions,
    dev_options: BindingDevOptions,
  ) -> napi::Result<Self> {
    crate::memory_reclaim::register_js_thread(&env);
    let session_id = rolldown_devtools::generate_session_id();
    let session = rolldown_devtools::Session::dummy();

    let on_hmr_updates_callback = dev_options.on_hmr_updates;
    let on_output_callback = dev_options.on_output;
    let on_additional_assets_callback = dev_options.on_additional_assets;

    let cwd: Arc<Path> = Arc::from(PathBuf::from(options.input_options.cwd.clone()));

    let rebuild_strategy = dev_options.rebuild_strategy.map(Into::into);
    let hot_update = dev_options.hot_update;
    let dev_watch_options = dev_options.watch.map(Into::into);

    // Create bundler config
    let normalized = create_bundler_config_from_binding_options(options)?;
    let (bundler_config, option_callbacks) =
      (normalized.config, SharedJsCallbackRelease::new(vec![normalized.callbacks]));

    // If callback is provided, wrap it to convert BuildResult<(HmrUpdate, Vec<String>)> to BindingResult<(BindingHmrUpdate, Vec<String>)>
    let on_hmr_updates = on_hmr_updates_callback.map(|js_callback| {
      let cwd = Arc::<Path>::clone(&cwd);
      Arc::new(
        move |result: rolldown_error::BuildResult<(rolldown_common::HmrUpdate, Vec<String>)>| {
          let binding_result: BindingResult<(BindingHmrUpdate, Vec<String>)> = match result {
            Ok((update, changed_files)) => {
              Either::B((BindingHmrUpdate::from(update), changed_files))
            }
            Err(errors) => {
              let binding_errors: Vec<_> = errors
                .iter()
                .map(|diagnostic| to_binding_error(diagnostic, cwd.to_path_buf()))
                .collect();
              Either::A(BindingErrors::new(binding_errors))
            }
          };
          js_callback
            .call(FnArgs { data: (binding_result,) }, ThreadsafeFunctionCallMode::Blocking);
        },
      ) as OnHmrUpdatesCallback
    });

    // If callback is provided, wrap it to convert BuildResult<BundleOutput> to BindingResult<BindingOutputs>
    let on_output = on_output_callback.map(|js_callback| {
      let cwd = Arc::<Path>::clone(&cwd);
      Arc::new(move |result: rolldown_error::BuildResult<rolldown::BundleOutput>| {
        let mut hmr_generation = None;
        let binding_result: BindingResult<BindingOutputs> = match result {
          Ok(bundle_output) => {
            hmr_generation = bundle_output.hmr_generation;
            Either::B(BindingOutputs::from(bundle_output))
          }
          Err(errors) => {
            let binding_errors: Vec<_> = errors
              .iter()
              .map(|diagnostic| to_binding_error(diagnostic, cwd.to_path_buf()))
              .collect();
            Either::A(BindingErrors::new(binding_errors))
          }
        };
        js_callback.call(
          FnArgs { data: (binding_result, hmr_generation) },
          ThreadsafeFunctionCallMode::Blocking,
        );
      }) as OnOutputCallback
    });

    // Assets emitted during an HMR patch / lazy compile (these never go through
    // `on_output`). Forward the assets; warnings stay Rust-side, as in `on_output`.
    let on_additional_assets = on_additional_assets_callback.map(|js_callback| {
      Arc::new(move |output: rolldown::BundleOutput| {
        let binding_outputs = BindingOutputs::from(output);
        js_callback.call(FnArgs { data: (binding_outputs,) }, ThreadsafeFunctionCallMode::Blocking);
      }) as OnAdditionalAssetsCallback
    });

    let rolldown_dev_options = rolldown_dev::DevOptions {
      on_hmr_updates,
      on_output,
      on_additional_assets,
      rebuild_strategy,
      watch: dev_watch_options,
      hot_update,
    };

    let inner = rolldown_dev::DevEngine::new(bundler_config, rolldown_dev_options)
      .map_err(|e| napi::Error::from_reason(format!("Fail to create dev engine: {e:#?}")))?;

    Ok(Self {
      inner: Arc::new(inner),
      cwd,
      _session_id: session_id,
      _session: session,
      option_callbacks,
    })
  }

  #[napi(ts_return_type = "Promise<void>")]
  pub fn run<'env>(&self, env: &'env Env) -> napi::Result<PromiseRaw<'env, ()>> {
    let inner = Arc::clone(&self.inner);
    spawn_boxed_future(env, async move {
      inner.run().await.map_err(|_e| napi::Error::from_reason("Failed to run dev engine"))?;
      Ok(())
    })
  }

  #[napi(ts_return_type = "Promise<void>")]
  pub fn ensure_current_build_finish<'env>(
    &self,
    env: &'env Env,
  ) -> napi::Result<PromiseRaw<'env, ()>> {
    let inner = Arc::clone(&self.inner);
    spawn_boxed_future(env, async move {
      inner
        .wait_for_ongoing_bundle()
        .await
        .map_err(|_e| napi::Error::from_reason("Failed to ensure current build finish"))?;
      Ok(())
    })
  }

  #[napi]
  pub fn get_bundle_state<'env>(
    &self,
    env: &'env Env,
  ) -> napi::Result<PromiseRaw<'env, BindingBundleState>> {
    let inner = Arc::clone(&self.inner);
    spawn_boxed_future(env, async move {
      inner
        .get_bundle_state()
        .await
        .map(Into::into)
        .map_err(|_e| napi::Error::from_reason("Failed to get bundle state"))
    })
  }

  #[napi]
  pub fn ensure_latest_build_output<'env>(
    &self,
    env: &'env Env,
  ) -> napi::Result<PromiseRaw<'env, BindingResult<()>>> {
    let inner = Arc::clone(&self.inner);
    let cwd = Arc::clone(&self.cwd);
    spawn_boxed_future(env, async move {
      match inner.ensure_latest_bundle_output().await {
        Ok(()) => Ok(Either::B(())),
        Err(errors) => {
          let binding_errors: Vec<_> = errors
            .iter()
            .map(|diagnostic| to_binding_error(diagnostic, cwd.to_path_buf()))
            .collect();
          Ok(Either::A(BindingErrors::new(binding_errors)))
        }
      }
    })
  }

  #[napi]
  pub fn trigger_full_build(&self) {
    self.inner.trigger_full_build().expect("Should handle this error");
  }

  /// Renders the patch a client asks for. `query` is the query string of the client's patch
  /// request: `s=<epoch>&g=<gen>&u=<set>&e=<set>&f=<set>` (other keys are ignored). Each set is
  /// `b` and a base64url bitset (`HmrNumSet` in `rolldown_common`). A `Reload` answer means the
  /// client must reload; a bad query gets one too.
  #[napi]
  pub fn render_patch<'env>(
    &self,
    env: &'env Env,
    query: String,
  ) -> napi::Result<PromiseRaw<'env, BindingResult<BindingPatchResponse>>> {
    let inner = Arc::clone(&self.inner);
    let cwd = Arc::<Path>::clone(&self.cwd);
    spawn_boxed_future(env, async move {
      Ok(match inner.render_patch(&query).await {
        Ok(HmrPullResponse::Payload(patch)) => {
          Either::B(BindingPatchResponse::Patch { code: patch.code, sourcemap: patch.sourcemap })
        }
        Ok(HmrPullResponse::Reload { reason }) => {
          Either::B(BindingPatchResponse::Reload { reason })
        }
        Err(errors) => Either::A(BindingErrors::new(
          errors.iter().map(|diagnostic| to_binding_error(diagnostic, cwd.to_path_buf())).collect(),
        )),
      })
    })
  }

  #[napi(ts_return_type = "Promise<void>")]
  pub fn close<'env>(&self, env: &'env Env) -> napi::Result<PromiseRaw<'env, ()>> {
    let inner = Arc::clone(&self.inner);
    // A repeated `DevEngine::close` returns at once. This is also true inside a `closeBundle`
    // hook of the first close. So each call takes a hold before the spawn. The release waits for
    // the last hold, so it runs when the first close ends. The repeated close does not wait,
    // because a wait deadlocks when `closeBundle` awaits `engine.close()`.
    let hold = self.option_callbacks.hold();
    let option_callbacks = self.option_callbacks.clone();
    spawn_boxed_future(env, async move {
      // Rust drops locals in reverse order. So the hold drops first, and then the release
      // request runs.
      let _release = RequestReleaseOnDrop(option_callbacks);
      let _hold = hold;
      inner.close().await.map_err(|_e| napi::Error::from_reason("Failed to close dev engine"))?;
      Ok(())
    })
  }

  /// Compile a lazy entry module and return HMR-style patch code. `query` describes what the
  /// client holds, in the format of `renderPatch` (`u` is not used).
  ///
  /// This is called when a dynamically imported module is first requested at runtime.
  /// The module was previously stubbed with a proxy, and now we need to compile the
  /// actual module and its dependencies.
  /// Failures come back as `BindingErrors`, the same shape the build and HMR callbacks
  /// use, rather than as a napi error carrying a formatted string. A lazy compile runs the
  /// full plugin pipeline, so what fails here is usually a user plugin - and `to_binding_error`
  /// hands its thrown JS error back untouched, keeping the stack and the diagnostic's module
  /// id instead of flattening both into a message.
  #[napi]
  pub fn compile_entry<'env>(
    &self,
    env: &'env Env,
    module_id: String,
    query: String,
  ) -> napi::Result<PromiseRaw<'env, BindingResult<BindingLazyChunkResponse>>> {
    let inner = Arc::clone(&self.inner);
    let cwd = Arc::<Path>::clone(&self.cwd);
    spawn_boxed_future(env, async move {
      Ok(match inner.compile_lazy_entry(module_id, &query).await {
        Ok(HmrPullResponse::Payload(output)) => Either::B(BindingLazyChunkResponse::Chunk {
          code: output.code,
          sourcemap: output.sourcemap,
        }),
        Ok(HmrPullResponse::Reload { reason }) => {
          Either::B(BindingLazyChunkResponse::Reload { reason })
        }
        Err(errors) => Either::A(BindingErrors::new(
          errors.iter().map(|diagnostic| to_binding_error(diagnostic, cwd.to_path_buf())).collect(),
        )),
      })
    })
  }

  /// Same data the plugin-context `getModuleInfo` returns, readable from the engine
  /// handle at any time (no hook context needed).
  #[napi]
  pub fn get_module_info(&self, module_id: String) -> Option<BindingModuleInfo> {
    self.inner.get_module_info(&module_id).map(BindingModuleInfo::new)
  }

  #[napi]
  pub fn get_module_ids<'env>(&self, env: &'env Env) -> napi::Result<Vec<napi::JsString<'env>>> {
    self.inner.get_module_ids().iter().map(|id| env.create_string(id)).try_collect()
  }
}

/// With `sourcemap` set to `Inline`, `code` ends with an inline `sourceMappingURL`. With `File`
/// or `Hidden`, `sourcemap` is the map as JSON, and `code` has no `sourceMappingURL` comment: no
/// file is kept for a pulled payload, so the consumer serves the map and adds the comment.
#[napi(discriminant = "type", object_from_js = false)]
pub enum BindingPatchResponse {
  Patch {
    code: String,
    sourcemap: Option<String>,
  },
  /// The client must reload: it belongs to another engine, its generation is too old to catch
  /// up, or the query is bad.
  Reload {
    reason: String,
  },
}

/// `code` and `sourcemap` follow the rules of `BindingPatchResponse`.
#[napi(discriminant = "type", object_from_js = false)]
pub enum BindingLazyChunkResponse {
  Chunk { code: String, sourcemap: Option<String> },
  Reload { reason: String },
}

#[napi(object)]
pub struct BindingBundleState {
  pub last_build_errored: bool,
  /// The stage of the last incremental failure, when `last_build_errored`
  /// is true and the engine is in an incremental-failure state. Absent on
  /// success and for an initial full-build failure (use
  /// `last_build_errored` to detect that). The consumer can force a full
  /// rebuild on the next page load when this is `Hmr`. See
  /// `internal-docs/dev-engine/implementation.md` §12.
  pub last_error_stage: Option<BindingErrorStage>,
  pub has_stale_output: bool,
}

impl From<BundleState> for BindingBundleState {
  fn from(state: BundleState) -> Self {
    Self {
      last_build_errored: state.last_build_errored,
      last_error_stage: state.last_error_stage.map(Into::into),
      has_stale_output: state.has_stale_output,
    }
  }
}
