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

#[cfg(all(target_family = "wasm", tokio_unstable))]
use std::sync::{
  LazyLock,
  atomic::{AtomicU32, Ordering},
};
#[cfg(not(target_family = "wasm"))]
use std::{
  sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
  },
  time::Instant,
};

use napi_derive::napi;

#[cfg(all(
  not(target_family = "wasm"),
  not(feature = "default_global_allocator"),
  not(target_env = "ohos")
))]
#[global_allocator]
static ALLOC: mimalloc_safe::MiMalloc = mimalloc_safe::MiMalloc;

pub mod binding_bundler;
pub mod binding_dev_engine;
pub mod binding_dev_options;
pub mod binding_watcher_bundler;
pub mod classic_bundler;
mod generated;
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

#[cfg(all(target_family = "wasm", tokio_unstable))]
pub static ACTIVE_TASK_COUNT: LazyLock<AtomicU32> = LazyLock::new(|| AtomicU32::new(1));

#[cfg(not(target_family = "wasm"))]
static MODULE_INIT_METRICS_ORDINAL: AtomicU64 = AtomicU64::new(0);

#[napi]
/// Shutdown the tokio runtime manually.
///
/// This is required for the wasm target with `tokio_unstable` cfg.
/// In the wasm runtime, the `park` threads will hang there until the tokio::Runtime is shutdown.
pub fn shutdown_async_runtime() {
  #[cfg(all(target_family = "wasm", tokio_unstable))]
  {
    if ACTIVE_TASK_COUNT.load(Ordering::Relaxed) > 0 {
      if ACTIVE_TASK_COUNT.fetch_sub(1, Ordering::Relaxed) == 1 {
        napi::bindgen_prelude::shutdown_async_runtime();
      }
    }
  }
}

#[napi]
/// Start the async runtime manually.
///
/// This is required when the async runtime is shutdown manually.
/// Usually it's used in test.
pub fn start_async_runtime() {
  #[cfg(all(target_family = "wasm", tokio_unstable))]
  {
    napi::bindgen_prelude::start_async_runtime();
    ACTIVE_TASK_COUNT.fetch_add(1, Ordering::Relaxed);
  }
}

#[napi_derive::module_init]
fn init() {
  #[cfg(not(target_family = "wasm"))]
  {
    use napi::{bindgen_prelude::create_custom_tokio_runtime, tokio};
    let metrics_enabled =
      std::env::var("ROLLDOWN_PARALLEL_PLUGIN_METRICS").as_deref() == Ok("json");
    let metrics_ordinal =
      metrics_enabled.then(|| MODULE_INIT_METRICS_ORDINAL.fetch_add(1, Ordering::Relaxed) + 1);
    let metrics_started_at = metrics_enabled.then(Instant::now);
    let started_threads = metrics_enabled.then(|| Arc::new(AtomicU64::new(0)));
    let stopped_threads = metrics_enabled.then(|| Arc::new(AtomicU64::new(0)));
    let max_blocking_threads = std::env::var("ROLLDOWN_MAX_BLOCKING_THREADS")
      .ok()
      .and_then(|v| v.parse::<usize>().ok())
      // default value in tokio implementation is **512**
      // it's too high for us
      // we don't have that many `blocking` tasks to run at this moment
      .unwrap_or(4);
    let worker_threads = std::env::var("ROLLDOWN_WORKER_THREADS")
      .ok()
      .and_then(|v| v.parse::<usize>().ok())
      // unlike the web server scenario
      // rolldown puts a lot of blocking tasks in the worker threads rather than blocking_threads
      // so we need to increase the worker threads rather than the blocking_threads
      .unwrap_or(num_cpus::get_physical() * 3 / 2);
    let mut builder = tokio::runtime::Builder::new_multi_thread();
    if let (Some(started_threads), Some(stopped_threads)) =
      (started_threads.as_ref(), stopped_threads.as_ref())
    {
      let started_threads = Arc::clone(started_threads);
      builder.on_thread_start(move || {
        started_threads.fetch_add(1, Ordering::Relaxed);
      });
      let stopped_threads = Arc::clone(stopped_threads);
      builder.on_thread_stop(move || {
        stopped_threads.fetch_add(1, Ordering::Relaxed);
      });
    }

    let rt = builder
      .max_blocking_threads(max_blocking_threads)
      .worker_threads(worker_threads)
      .thread_name("rolldown-worker")
      .enable_all()
      .build()
      .expect("Failed to create tokio runtime");
    let runtime_built_at = metrics_started_at.map(|started_at| started_at.elapsed());
    let threads_started_after_build =
      started_threads.as_ref().map(|value| value.load(Ordering::Relaxed));
    let threads_stopped_after_build =
      stopped_threads.as_ref().map(|value| value.load(Ordering::Relaxed));
    create_custom_tokio_runtime(rt);
    if let Some(started_at) = metrics_started_at {
      let completed_at = started_at.elapsed();
      let report = serde_json::json!({
        "kind": "rolldown_binding_module_init_metrics",
        "version": 1,
        "invocationOrdinal": metrics_ordinal,
        "configuredTokioWorkerThreads": worker_threads,
        "configuredTokioMaxBlockingThreads": max_blocking_threads,
        "runtimeBuildMs": runtime_built_at.map(|duration| duration.as_secs_f64() * 1_000.0),
        "customRuntimeRegistrationMs": runtime_built_at
          .map(|runtime_built_at| completed_at.saturating_sub(runtime_built_at).as_secs_f64() * 1_000.0),
        "totalMs": completed_at.as_secs_f64() * 1_000.0,
        "threadsStartedAfterBuild": threads_started_after_build,
        "threadsStoppedAfterBuild": threads_stopped_after_build,
        "threadsStartedAfterRegistration": started_threads
          .as_ref()
          .map(|value| value.load(Ordering::Relaxed)),
        "threadsStoppedAfterRegistration": stopped_threads
          .as_ref()
          .map(|value| value.load(Ordering::Relaxed)),
        "interpretation": "Per-invocation Tokio callback counts. A later invocation that starts and stops its configured threads during registration constructed a runtime that was not retained by napi's process-global custom runtime slot."
      });
      eprintln!("[rolldown-parallel-plugin-module-init-metrics] {report}");
    }
  }

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
