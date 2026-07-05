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

use napi_derive::napi;

pub mod async_runtime;
mod env_config;

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

#[cfg(any(test, all(target_family = "wasm", tokio_unstable)))]
mod async_runtime_lease {
  use std::sync::Mutex;

  use napi::{Error, Status};

  pub struct Manager {
    active_task_count: Mutex<u32>,
  }

  impl Manager {
    pub const fn new(active_task_count: u32) -> Self {
      Self { active_task_count: Mutex::new(active_task_count) }
    }

    pub fn acquire(&self, start: impl FnOnce() -> napi::Result<()>) -> napi::Result<()> {
      let mut active_task_count =
        self.active_task_count.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
      let next_count = active_task_count.checked_add(1).ok_or_else(|| {
        Error::new(Status::GenericFailure, "Async runtime owner count overflowed")
      })?;
      if *active_task_count == 0 {
        start()?;
      }
      *active_task_count = next_count;
      Ok(())
    }

    pub fn release(&self, shutdown: impl FnOnce() -> napi::Result<()>) -> napi::Result<()> {
      let mut active_task_count =
        self.active_task_count.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
      match *active_task_count {
        0 => Ok(()),
        1 => {
          shutdown()?;
          *active_task_count = 0;
          Ok(())
        }
        _ => {
          *active_task_count -= 1;
          Ok(())
        }
      }
    }

    #[cfg(test)]
    fn active_task_count(&self) -> u32 {
      *self.active_task_count.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }
  }

  #[cfg(test)]
  mod tests {
    use std::sync::{
      Arc,
      atomic::{AtomicUsize, Ordering},
    };

    use super::Manager;

    #[test]
    fn release_is_idempotent_after_the_last_owner() {
      let manager = Manager::new(1);
      let shutdown_calls = AtomicUsize::new(0);

      manager
        .release(|| {
          shutdown_calls.fetch_add(1, Ordering::SeqCst);
          Ok(())
        })
        .unwrap();
      manager
        .release(|| {
          shutdown_calls.fetch_add(1, Ordering::SeqCst);
          Ok(())
        })
        .unwrap();

      assert_eq!(manager.active_task_count(), 0);
      assert_eq!(shutdown_calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn failed_start_and_shutdown_preserve_the_transition_owner() {
      let stopped = Manager::new(0);
      let start_error =
        stopped.acquire(|| Err(napi::Error::from_reason("start failed"))).unwrap_err();
      assert_eq!(start_error.reason, "start failed");
      assert_eq!(stopped.active_task_count(), 0);
      stopped.acquire(|| Ok(())).unwrap();
      assert_eq!(stopped.active_task_count(), 1);

      let shutdown_error =
        stopped.release(|| Err(napi::Error::from_reason("shutdown failed"))).unwrap_err();
      assert_eq!(shutdown_error.reason, "shutdown failed");
      assert_eq!(stopped.active_task_count(), 1);
      stopped.release(|| Ok(())).unwrap();
      assert_eq!(stopped.active_task_count(), 0);
    }

    #[test]
    fn concurrent_release_never_underflows() {
      let manager = Arc::new(Manager::new(64));
      let shutdown_calls = Arc::new(AtomicUsize::new(0));
      let threads = (0..128)
        .map(|_| {
          let manager = Arc::clone(&manager);
          let shutdown_calls = Arc::clone(&shutdown_calls);
          std::thread::spawn(move || {
            manager
              .release(|| {
                shutdown_calls.fetch_add(1, Ordering::SeqCst);
                Ok(())
              })
              .unwrap();
          })
        })
        .collect::<Vec<_>>();

      for thread in threads {
        thread.join().unwrap();
      }

      assert_eq!(manager.active_task_count(), 0);
      assert_eq!(shutdown_calls.load(Ordering::SeqCst), 1);
    }
  }
}

#[cfg(all(target_family = "wasm", tokio_unstable))]
// See internal-docs/async-runtime/implementation.md.
static ASYNC_RUNTIME_LEASES: async_runtime_lease::Manager = async_runtime_lease::Manager::new(1);

#[cfg(not(all(target_family = "wasm", tokio_unstable)))]
#[expect(clippy::unnecessary_wraps, reason = "matches the fallible threaded-WASI export")]
fn no_op_async_runtime_transition() -> napi::Result<()> {
  Ok(())
}

#[napi]
/// Shutdown the tokio runtime manually.
///
/// This is required for the wasm target with `tokio_unstable` cfg.
/// In the wasm runtime, the `park` threads will hang there until the tokio::Runtime is shutdown.
pub fn shutdown_async_runtime() -> napi::Result<()> {
  #[cfg(all(target_family = "wasm", tokio_unstable))]
  {
    return ASYNC_RUNTIME_LEASES.release(napi::bindgen_prelude::try_shutdown_async_runtime);
  }
  #[cfg(not(all(target_family = "wasm", tokio_unstable)))]
  no_op_async_runtime_transition()
}

#[napi]
/// Start the async runtime manually.
///
/// This is required when the async runtime is shutdown manually.
/// Usually it's used in test.
pub fn start_async_runtime() -> napi::Result<()> {
  #[cfg(all(target_family = "wasm", tokio_unstable))]
  {
    return ASYNC_RUNTIME_LEASES.acquire(napi::bindgen_prelude::try_start_async_runtime);
  }
  #[cfg(not(all(target_family = "wasm", tokio_unstable)))]
  no_op_async_runtime_transition()
}

#[napi_derive::module_init]
fn init() {
  // Pin the runtime-config snapshot at module load on EVERY artifact, not
  // just the ones that build a Rust runtime below. The threaded-WASI tokio
  // artifact builds none, but its JS loader sizes the real async work pool
  // from the environment at module load -- resolving lazily there would let
  // a post-import env change make the report diverge from the pool that is
  // already running (and would leave the pinning to the accident of the
  // host's WASI shim snapshotting its env). One source of truth: resolve
  // here, where every consumer of the snapshot agrees on "load time".
  crate::async_runtime::resolved_runtime_config();

  #[cfg(all(
    not(target_family = "wasm"),
    feature = "tokio-runtime",
    not(feature = "async-runtime")
  ))]
  {
    use napi::{bindgen_prelude::create_custom_tokio_runtime, tokio};
    // Build the tokio runtime from the SAME resolved snapshot the diagnostics
    // reporter (`get_async_runtime_config`) and `get_runtime_capabilities`
    // serve -- the single config-resolution pipeline -- so the reported
    // config always matches the runtime actually built here. The measured
    // defaults (worker threads at physical * 3 / 2, a dedicated 4-thread
    // blocking pool instead of tokio's 512) live in the resolver's
    // per-(backend, target) table; see async_runtime.rs.
    let resolved = crate::async_runtime::resolved_runtime_config();
    let mut builder = tokio::runtime::Builder::new_multi_thread();

    let rt = builder
      .max_blocking_threads(resolved.max_blocking_tasks)
      .worker_threads(resolved.worker_threads)
      .thread_name("rolldown-worker")
      .enable_all()
      .build()
      .expect("Failed to create tokio runtime");
    create_custom_tokio_runtime(rt);
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
