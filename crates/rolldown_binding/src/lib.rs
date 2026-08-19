#![expect(clippy::print_stderr)]
// Allow type complexity rule, because NAPI-RS requires the direct types to generate the TypeScript definitions.
#![allow(clippy::type_complexity)]
// Due to the bound of NAPI-RS, we need to use `String` though we only need `&str`.
#![allow(clippy::needless_pass_by_value)]
// Most of transmute are just change the lifetime `'a` to `'static`., the annotation, e.g.
//
// BindingTransformPluginContext::new(unsafe {
//   std::mem::transmute::<
//     &rolldown_plugin::TransformPluginContext<'_>,
//     &rolldown_plugin::TransformPluginContext<'_>,
//   >(ctx)
// }),
// Looks redundant
#![allow(clippy::missing_transmute_annotations)]
// NAPI-RS requires `std::collections::HashMap`/`HashSet` to generate the TypeScript definitions,
// so the whole binding crate opts out of the `FxHashMap`/`FxHashSet` type ban (the hasher is
// already `FxBuildHasher` at every use site).
#![allow(clippy::disallowed_types)]

// `.github/workflows/reusable-wasi.yml` greps for this exact message; keep the
// two in sync.
#[cfg(not(feature = "async-runtime"))]
compile_error!(
  "rolldown_binding requires the `async-runtime` feature: the shared tokio-free scheduler is the only runtime"
);

use napi_derive::napi;

pub mod async_runtime;
mod env_config;

#[cfg(all(
  not(target_family = "wasm"),
  not(feature = "default_global_allocator"),
  not(target_env = "ohos"),
  not(feature = "tracking_allocator")
))]
#[global_allocator]
static ALLOC: mimalloc_safe::MiMalloc = mimalloc_safe::MiMalloc;

// Same mimalloc, wrapped with allocation counters. The counters are read
// through `getNativeMemoryStats()` in `native_memory.rs`; the cfg there must
// stay in sync with this one.
#[cfg(all(
  not(target_family = "wasm"),
  not(feature = "default_global_allocator"),
  not(target_env = "ohos"),
  feature = "tracking_allocator"
))]
#[global_allocator]
static ALLOC: rolldown_tracking_allocator::TrackingAllocator =
  rolldown_tracking_allocator::TrackingAllocator;

pub mod binding_bundler;
pub mod binding_dev_engine;
pub mod binding_dev_options;
pub mod binding_watcher_bundler;
pub mod classic_bundler;
mod generated;
pub mod native_memory;
pub mod options;
pub mod parallel_js_plugin_registry;
pub mod transform;
pub mod transform_cache;
pub mod types;
pub mod utils;
pub mod watcher;
pub mod worker_manager;

// --- External NAPI-RS dependencies ---
pub use oxc_parser_napi;
pub use oxc_resolver_napi;

/// A compatibility no-op: the async runtime's lifecycle follows the N-API
/// environment, so `release()` does nothing. Kept because the generated WASI
/// loaders still acquire a lease at import and release it at teardown.
#[napi]
pub struct BindingAsyncRuntimeLease {}

#[napi]
impl BindingAsyncRuntimeLease {
  #[napi]
  pub fn release(&self) {}
}

pub struct AcquireAsyncRuntimeTask {}

#[napi]
impl napi::Task for AcquireAsyncRuntimeTask {
  type Output = ();
  type JsValue = BindingAsyncRuntimeLease;

  fn compute(&mut self) -> napi::Result<Self::Output> {
    Ok(())
  }

  fn resolve(&mut self, _env: napi::Env, (): Self::Output) -> napi::Result<Self::JsValue> {
    Ok(BindingAsyncRuntimeLease {})
  }
}

#[napi]
/// Acquire an async runtime lifecycle lease. See `BindingAsyncRuntimeLease`:
/// the lease is a no-op.
pub fn acquire_async_runtime(
  _env: &napi::Env,
) -> napi::bindgen_prelude::AsyncTask<AcquireAsyncRuntimeTask> {
  napi::bindgen_prelude::AsyncTask::new(AcquireAsyncRuntimeTask {})
}

#[napi]
/// A no-op kept for compatibility; the async runtime follows the N-API
/// environment lifecycle.
pub fn shutdown_async_runtime() {}

#[napi]
/// A no-op kept for compatibility; the async runtime follows the N-API
/// environment lifecycle.
pub fn start_async_runtime() {}

#[cfg(test)]
mod manual_async_runtime_transition_tests {
  #[test]
  fn manual_lifecycle_exports_are_noops() {
    super::start_async_runtime();
    super::shutdown_async_runtime();
  }
}

#[napi_derive::module_init]
fn init() {
  // Pin the runtime-config snapshot at module load: the WASI JS loaders size
  // the real emnapi async work pool from the environment at load time, so a
  // lazy resolve could report a config the already-running pool does not match.
  crate::async_runtime::resolved_runtime_config();

  #[cfg(not(feature = "disable_panic_hook"))]
  {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
      eprintln!("Rolldown panicked. This is a bug in Rolldown, not your code.");
      default_hook(info);
      eprintln!(
        "\nPlease report this issue at: https://github.com/rolldown/rolldown/issues/new?template=panic_report.yml"
      );
    }));
  }
}
