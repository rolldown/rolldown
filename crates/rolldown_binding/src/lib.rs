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

#[cfg(all(target_family = "wasm", tokio_unstable))]
use std::sync::{
  LazyLock,
  atomic::{AtomicU32, Ordering},
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
    // Bundling parallelism now runs on rayon (module tasks, scope, codegen,
    // minification all use rayon::par_iter / rayon::spawn). Honour
    // ROLLDOWN_WORKER_THREADS here by sizing rayon's global pool — that's
    // the threadpool that actually does the work. Default keeps the
    // previous tuning (1.5x physical cores, biased high because rolldown
    // does a lot of mostly-CPU work with short awaits).
    let worker_threads = std::env::var("ROLLDOWN_WORKER_THREADS")
      .ok()
      .and_then(|v| v.parse::<usize>().ok())
      .unwrap_or(num_cpus::get_physical() * 3 / 2);
    // build_global() returns Err if the global pool was already initialised
    // (e.g. the host process configured it before loading rolldown). That
    // case is fine — we leave the existing pool alone instead of panicking.
    let _ = rayon::ThreadPoolBuilder::new()
      .num_threads(worker_threads)
      .thread_name(|i| format!("rolldown-worker-{i}"))
      .build_global();

    // napi-rs still backs `#[napi] async fn` Promises with its own tokio
    // runtime; we no longer need to customise its worker-thread count
    // because the Promise futures themselves don't do heavy work — they
    // just await calls into rolldown which runs on rayon.
    // ROLLDOWN_MAX_BLOCKING_THREADS used to size tokio's blocking pool;
    // after the Phase 3 refactor nothing in the build path calls
    // tokio::task::spawn_blocking, so it's no longer wired up.
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
