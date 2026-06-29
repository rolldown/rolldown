// The public napi surface in this module (the `Binding*` `#[napi(object)]` types and
// the `configure_async_runtime` / `get_async_runtime_*` / `reset_async_runtime_metrics`
// `#[napi]` exports) is reachable only from JS. The in-crate unit-test binary never
// constructs or calls it, so `dead_code` flags it in the TEST profile under every
// feature combination (the `async-runtime` arms when the feature is on, the stub arms
// when it is off). Relax dead_code for the TEST profile only: genuinely dead library
// code is still caught by the non-test (cdylib) clippy gate, which carries no such allow.
#![cfg_attr(test, allow(dead_code))]

#[cfg(feature = "async-runtime")]
use std::{future::Future, pin::Pin};

#[cfg(feature = "async-runtime")]
use napi::bindgen_prelude::{AsyncRuntime, create_custom_async_runtime};
use napi_derive::napi;
#[cfg(feature = "async-runtime")]
use rolldown_utils::async_runtime::{
  RuntimeFlavor, RuntimeMetricsSnapshot, RuntimeOptions, block_on_dyn, configure,
  configured_options, metrics, reset_metrics, shutdown, spawn_detached,
};

#[cfg(feature = "async-runtime")]
struct RolldownAsyncRuntime;

#[cfg(feature = "async-runtime")]
impl AsyncRuntime for RolldownAsyncRuntime {
  fn spawn(&self, future: Pin<Box<dyn Future<Output = ()> + Send + 'static>>) {
    spawn_detached(future);
  }

  fn block_on(&self, future: Pin<&mut dyn Future<Output = ()>>) {
    block_on_dyn(future);
  }

  fn shutdown(&self) {
    shutdown();
  }
}

#[napi(string_enum)]
#[derive(Clone, Copy)]
pub enum BindingRuntimeFlavor {
  CurrentThread,
  MultiThread,
}

#[cfg(feature = "async-runtime")]
impl From<BindingRuntimeFlavor> for RuntimeFlavor {
  fn from(value: BindingRuntimeFlavor) -> Self {
    match value {
      BindingRuntimeFlavor::CurrentThread => Self::CurrentThread,
      BindingRuntimeFlavor::MultiThread => Self::MultiThread,
    }
  }
}

#[cfg(feature = "async-runtime")]
impl From<RuntimeFlavor> for BindingRuntimeFlavor {
  fn from(value: RuntimeFlavor) -> Self {
    match value {
      RuntimeFlavor::CurrentThread => Self::CurrentThread,
      RuntimeFlavor::MultiThread => Self::MultiThread,
    }
  }
}

#[napi(object)]
pub struct BindingRuntimeOptions {
  pub flavor: Option<BindingRuntimeFlavor>,
  pub worker_threads: Option<u32>,
  pub max_blocking_tasks: Option<u32>,
}

#[napi(object)]
pub struct BindingRuntimeConfig {
  pub flavor: BindingRuntimeFlavor,
  pub worker_threads: u32,
  pub max_blocking_tasks: u32,
}

#[cfg(feature = "async-runtime")]
impl From<RuntimeOptions> for BindingRuntimeConfig {
  fn from(value: RuntimeOptions) -> Self {
    Self {
      flavor: value.flavor.into(),
      worker_threads: saturating_u32(value.worker_threads as u64),
      max_blocking_tasks: saturating_u32(value.max_blocking_tasks as u64),
    }
  }
}

#[napi(object)]
pub struct BindingRuntimeMetrics {
  pub flavor: BindingRuntimeFlavor,
  pub worker_threads: u32,
  pub max_blocking_tasks: u32,
  pub tasks_spawned: u32,
  pub tasks_completed: u32,
  pub tasks_panicked: u32,
  pub runnable_schedules: u32,
  pub runnable_polls: u32,
  pub queued_runnables: u32,
  pub max_queued_runnables: u32,
  pub active_runnables: u32,
  pub max_active_runnables: u32,
  pub blocking_tasks_started: u32,
  pub blocking_tasks_completed: u32,
  pub active_blocking_tasks: u32,
  pub max_active_blocking_tasks: u32,
}

#[cfg(feature = "async-runtime")]
impl From<RuntimeMetricsSnapshot> for BindingRuntimeMetrics {
  fn from(value: RuntimeMetricsSnapshot) -> Self {
    Self {
      flavor: value.flavor.into(),
      worker_threads: saturating_u32(value.worker_threads as u64),
      max_blocking_tasks: saturating_u32(value.max_blocking_tasks as u64),
      tasks_spawned: saturating_u32(value.tasks_spawned),
      tasks_completed: saturating_u32(value.tasks_completed),
      tasks_panicked: saturating_u32(value.tasks_panicked),
      runnable_schedules: saturating_u32(value.runnable_schedules),
      runnable_polls: saturating_u32(value.runnable_polls),
      queued_runnables: saturating_u32(value.queued_runnables),
      max_queued_runnables: saturating_u32(value.max_queued_runnables),
      active_runnables: saturating_u32(value.active_runnables),
      max_active_runnables: saturating_u32(value.max_active_runnables),
      blocking_tasks_started: saturating_u32(value.blocking_tasks_started),
      blocking_tasks_completed: saturating_u32(value.blocking_tasks_completed),
      active_blocking_tasks: saturating_u32(value.active_blocking_tasks),
      max_active_blocking_tasks: saturating_u32(value.max_active_blocking_tasks),
    }
  }
}

fn saturating_u32(value: u64) -> u32 {
  u32::try_from(value).unwrap_or(u32::MAX)
}

#[cfg(feature = "async-runtime")]
#[napi]
/// Override the shared async runtime's flavor and thread counts.
///
/// Must be called before the first async binding call. On the default
/// `tokio-runtime` build this throws a feature-disabled error; only the
/// `async-runtime` build honors it.
pub fn configure_async_runtime(options: BindingRuntimeOptions) -> napi::Result<()> {
  let mut current = configured_options();
  if let Some(flavor) = options.flavor {
    current.flavor = flavor.into();
  }
  if let Some(worker_threads) = options.worker_threads {
    current.worker_threads = worker_threads as usize;
  }
  if let Some(max_blocking_tasks) = options.max_blocking_tasks {
    current.max_blocking_tasks = max_blocking_tasks as usize;
  }
  configure(current).map_err(|error| napi::Error::from_reason(error.to_string()))
}

#[cfg(not(feature = "async-runtime"))]
#[napi]
/// Override the shared async runtime's flavor and thread counts.
///
/// Must be called before the first async binding call. On the default
/// `tokio-runtime` build this throws a feature-disabled error; only the
/// `async-runtime` build honors it.
pub fn configure_async_runtime(options: BindingRuntimeOptions) -> napi::Result<()> {
  let _ = options;
  Err(napi::Error::from_reason(
    "This Rolldown binding was built without the `async-runtime` feature",
  ))
}

#[cfg(feature = "async-runtime")]
#[napi]
/// Return the effective async runtime configuration.
///
/// On the default `tokio-runtime` build the values are derived from the
/// environment variables and built-in defaults.
pub fn get_async_runtime_config() -> BindingRuntimeConfig {
  configured_options().into()
}

// Snapshot of the thread counts the NATIVE default (`tokio-runtime`) runtime was
// ACTUALLY built with at addon load (lib.rs `init`). `get_async_runtime_config`
// reports this snapshot rather than re-reading the environment on every call, so
// a later `process.env` mutation can no longer make the reported config diverge
// from the runtime that already exists (Finding A: diagnostics-accuracy; the
// runtime itself was always built once at init). Stored as resolved `u32`
// (worker_threads, max_blocking_tasks) -- the values fed to the tokio builder.
#[cfg(all(not(feature = "async-runtime"), not(target_family = "wasm")))]
static DEFAULT_RUNTIME_CONFIG: std::sync::OnceLock<(u32, u32)> = std::sync::OnceLock::new();

/// Resolve the native default runtime thread counts from the environment, using
/// the SAME variables and defaults the native tokio runtime is built with in
/// lib.rs `init`. Single source of truth so the snapshot stored at init exactly
/// matches the runtime that was constructed.
#[cfg(all(not(feature = "async-runtime"), not(target_family = "wasm")))]
pub fn resolve_default_runtime_threads() -> (usize, usize) {
  use crate::env_config::resolve_thread_count;
  let worker_threads = resolve_thread_count(
    std::env::var("ROLLDOWN_WORKER_THREADS").ok(),
    num_cpus::get_physical() * 3 / 2,
  );
  let max_blocking_tasks =
    resolve_thread_count(std::env::var("ROLLDOWN_MAX_BLOCKING_THREADS").ok(), 4);
  (worker_threads, max_blocking_tasks)
}

/// Record the resolved thread counts the native default runtime was built with.
/// Called once from lib.rs `init` after the tokio runtime is constructed. A
/// no-op if already set (the runtime is built exactly once per process).
#[cfg(all(not(feature = "async-runtime"), not(target_family = "wasm")))]
pub fn snapshot_default_runtime_config(worker_threads: usize, max_blocking_tasks: usize) {
  let _ = DEFAULT_RUNTIME_CONFIG
    .set((saturating_u32(worker_threads as u64), saturating_u32(max_blocking_tasks as u64)));
}

// Pure assembly of the native default reporter: prefer the init-time `snapshot`;
// only when it is absent (e.g. a build without the tokio runtime, or a unit test
// before init ran) fall back to resolving the env defaults. Split out so the
// snapshot-wins-over-later-env behaviour is unit-testable without touching
// process-global env or the `OnceLock`.
#[cfg(all(not(feature = "async-runtime"), not(target_family = "wasm")))]
fn default_runtime_config_from(
  snapshot: Option<(u32, u32)>,
  worker_threads_env: Option<String>,
  max_blocking_threads_env: Option<String>,
) -> BindingRuntimeConfig {
  use crate::env_config::resolve_thread_count;
  let (worker_threads, max_blocking_tasks) = match snapshot {
    Some((worker_threads, max_blocking_tasks)) => (worker_threads, max_blocking_tasks),
    None => {
      let worker_threads =
        resolve_thread_count(worker_threads_env, num_cpus::get_physical() * 3 / 2);
      let max_blocking_tasks = resolve_thread_count(max_blocking_threads_env, 4);
      (saturating_u32(worker_threads as u64), saturating_u32(max_blocking_tasks as u64))
    }
  };
  BindingRuntimeConfig {
    flavor: BindingRuntimeFlavor::MultiThread,
    worker_threads,
    max_blocking_tasks,
  }
}

// The size of the async work pool the napi-rs WASI loader actually creates on the
// threaded WASI artifact. Mirrors `packages/rolldown/src/rolldown-binding.wasi.cjs`:
//   Number(NAPI_RS_ASYNC_WORK_POOL_SIZE ?? UV_THREADPOOL_SIZE), used when `> 0`,
//   otherwise 4.
// `napi.or(uv)` reproduces the `??` precedence (the first key wins whenever it is
// present, even if empty/zero -- both then resolve to 4 on either side), and
// `resolve_thread_count` reproduces the loader's "non-positive / non-numeric =>
// default 4" guard for plain decimal integers (surrounding whitespace tolerated, to
// match `Number(" 5 ") == 5`). Exotic `Number` forms (scientific "1e2", hex "0x10",
// floats "1.5") are intentionally NOT mirrored: the loader's `Number()` and libuv's
// own `atoi` already disagree on them (atoi("1e2")=1, atoi("0x10")=0), so there is no
// single ground truth, nobody sets a thread-pool size that way, and this value is
// reported for diagnostics only -- the real pool is sized by the loader regardless.
// Compiled only where it is used (wasm) or tested (test), so the native cdylib build
// sees no dead code.
#[cfg(all(not(feature = "async-runtime"), any(target_family = "wasm", test)))]
fn wasm_async_work_pool_size(
  napi_async_work_pool_size_env: Option<String>,
  uv_threadpool_size_env: Option<String>,
) -> usize {
  use crate::env_config::resolve_thread_count;
  let selected =
    napi_async_work_pool_size_env.or(uv_threadpool_size_env).map(|value| value.trim().to_string());
  resolve_thread_count(selected, 4)
}

// wasm default reporter: there is no native tokio runtime on wasm (lib.rs `init`'s
// native arm is `not(target_family = "wasm")`), so no snapshot exists. The real
// async work pool is sized by the WASI loader from
// NAPI_RS_ASYNC_WORK_POOL_SIZE / UV_THREADPOOL_SIZE. Report that single pool
// honestly: the threaded WASI artifact is multi-thread, and both the worker and
// the blocking work run on that same loader-sized pool (there is no separate,
// Rust-controlled blocking pool to report a distinct number for).
#[cfg(all(not(feature = "async-runtime"), target_family = "wasm"))]
fn wasm_runtime_config() -> BindingRuntimeConfig {
  let pool = saturating_u32(wasm_async_work_pool_size(
    std::env::var("NAPI_RS_ASYNC_WORK_POOL_SIZE").ok(),
    std::env::var("UV_THREADPOOL_SIZE").ok(),
  ) as u64);
  BindingRuntimeConfig {
    flavor: BindingRuntimeFlavor::MultiThread,
    worker_threads: pool,
    max_blocking_tasks: pool,
  }
}

#[cfg(not(feature = "async-runtime"))]
#[napi]
/// Return the effective async runtime configuration.
///
/// On the native default `tokio-runtime` build this reports the thread counts the
/// runtime was ACTUALLY built with at addon load (snapshotted in lib.rs `init`),
/// so a later `process.env` change cannot make the report diverge from the live
/// runtime. On the threaded WASI build it reports the napi-rs WASI loader's async
/// work pool size (NAPI_RS_ASYNC_WORK_POOL_SIZE / UV_THREADPOOL_SIZE).
///
/// Scope: the snapshot is taken once per process (the runtime is built once in
/// `init`). If a host tears the env down and recreates it in the same process (an
/// Electron-style reload), napi-rs rebuilds its runtime with its OWN defaults and
/// the snapshot is not refreshed -- the same once-per-process lifecycle as napi's
/// env-cleanup hook. This is unchanged from before the snapshot (the prior
/// env-reading reporter could not reflect that napi-default runtime either); the
/// field is diagnostics-only, so it is left as-is rather than chasing the reload.
pub fn get_async_runtime_config() -> BindingRuntimeConfig {
  #[cfg(not(target_family = "wasm"))]
  {
    default_runtime_config_from(
      DEFAULT_RUNTIME_CONFIG.get().copied(),
      std::env::var("ROLLDOWN_WORKER_THREADS").ok(),
      std::env::var("ROLLDOWN_MAX_BLOCKING_THREADS").ok(),
    )
  }
  #[cfg(target_family = "wasm")]
  {
    wasm_runtime_config()
  }
}

#[cfg(feature = "async-runtime")]
#[napi]
/// Return a snapshot of the shared async runtime's task and scheduler counters.
///
/// On the default `tokio-runtime` build every counter is zero.
pub fn get_async_runtime_metrics() -> BindingRuntimeMetrics {
  metrics().into()
}

#[cfg(not(feature = "async-runtime"))]
#[napi]
/// Return a snapshot of the shared async runtime's task and scheduler counters.
///
/// On the default `tokio-runtime` build every counter is zero.
pub fn get_async_runtime_metrics() -> BindingRuntimeMetrics {
  let config = get_async_runtime_config();
  BindingRuntimeMetrics {
    flavor: config.flavor,
    worker_threads: config.worker_threads,
    max_blocking_tasks: config.max_blocking_tasks,
    tasks_spawned: 0,
    tasks_completed: 0,
    tasks_panicked: 0,
    runnable_schedules: 0,
    runnable_polls: 0,
    queued_runnables: 0,
    max_queued_runnables: 0,
    active_runnables: 0,
    max_active_runnables: 0,
    blocking_tasks_started: 0,
    blocking_tasks_completed: 0,
    active_blocking_tasks: 0,
    max_active_blocking_tasks: 0,
  }
}

#[cfg(feature = "async-runtime")]
#[napi]
/// Reset the async runtime metrics counters to zero.
///
/// A no-op on the default `tokio-runtime` build.
pub fn reset_async_runtime_metrics() {
  reset_metrics();
}

#[cfg(not(feature = "async-runtime"))]
#[napi]
/// Reset the async runtime metrics counters to zero.
///
/// A no-op on the default `tokio-runtime` build.
pub fn reset_async_runtime_metrics() {}

#[cfg(feature = "async-runtime")]
#[napi_derive::module_init]
fn register_async_runtime() {
  use crate::env_config::resolve_thread_count;

  let mut options = RuntimeOptions::default();
  #[cfg(not(target_family = "wasm"))]
  {
    options.worker_threads = resolve_thread_count(
      std::env::var("ROLLDOWN_WORKER_THREADS").ok(),
      num_cpus::get_physical(),
    );
  }
  options.max_blocking_tasks = resolve_thread_count(
    std::env::var("ROLLDOWN_MAX_BLOCKING_THREADS").ok(),
    options.worker_threads,
  );
  if let Ok(flavor) = std::env::var("ROLLDOWN_RUNTIME") {
    options.flavor = match flavor.as_str() {
      "current" | "current-thread" | "single" | "single-thread" => RuntimeFlavor::CurrentThread,
      "multi" | "multi-thread" => RuntimeFlavor::MultiThread,
      _ => options.flavor,
    };
  }
  configure(options).expect("Failed to configure the Rolldown async runtime");
  create_custom_async_runtime(RolldownAsyncRuntime);
}

#[cfg(all(test, not(feature = "async-runtime")))]
mod tests {
  // Finding A: the native default reporter must report the INIT-TIME snapshot,
  // not whatever the environment says at call time.
  #[cfg(not(target_family = "wasm"))]
  #[test]
  fn default_config_reports_init_snapshot_over_later_env() {
    use super::default_runtime_config_from;

    // With a snapshot present, the env values are IGNORED even though they ask
    // for completely different counts -- this is exactly the post-init reporter
    // path, which under the bug would re-read these mutated env values.
    let snapshot =
      default_runtime_config_from(Some((9, 3)), Some("99".to_string()), Some("88".to_string()));
    assert_eq!(
      (snapshot.worker_threads, snapshot.max_blocking_tasks),
      (9, 3),
      "the init-time snapshot must win over a later env mutation"
    );

    // Without a snapshot (pre-init / non-tokio build), the env IS honored. This
    // proves the env arguments are wired through and the assertion above is not
    // vacuous: absent the snapshot, the same env would change the output.
    let from_env = default_runtime_config_from(None, Some("7".to_string()), Some("5".to_string()));
    assert_eq!(
      (from_env.worker_threads, from_env.max_blocking_tasks),
      (7, 5),
      "without a snapshot the reporter resolves the env (so the snapshot truly overrides it)"
    );
  }

  // Finding B: the wasm reporter must size the pool from the SAME env keys and
  // precedence the napi-rs WASI loader uses, not a physical-cpu number.
  #[test]
  fn wasm_pool_size_matches_loader_env_precedence() {
    use super::wasm_async_work_pool_size;

    // NAPI_RS_ASYNC_WORK_POOL_SIZE wins when present (the `??` first operand).
    assert_eq!(
      wasm_async_work_pool_size(Some("6".to_string()), Some("2".to_string())),
      6,
      "NAPI_RS_ASYNC_WORK_POOL_SIZE must take precedence over UV_THREADPOOL_SIZE"
    );
    // Falls back to UV_THREADPOOL_SIZE when the first key is absent.
    assert_eq!(
      wasm_async_work_pool_size(None, Some("2".to_string())),
      2,
      "UV_THREADPOOL_SIZE must be used when NAPI_RS_ASYNC_WORK_POOL_SIZE is unset"
    );
    // Default of 4 when neither key is set (loader's else branch).
    assert_eq!(wasm_async_work_pool_size(None, None), 4, "default pool size is 4");
    // Non-positive / non-numeric values resolve to the default 4, mirroring the
    // loader's `Number(...) > 0 ? ... : 4` guard. A present-but-zero first key
    // still "wins" the `??` and then falls to 4 (never the UV value).
    assert_eq!(wasm_async_work_pool_size(Some("0".to_string()), Some("9".to_string())), 4);
    assert_eq!(wasm_async_work_pool_size(Some("abc".to_string()), None), 4);
    // Surrounding whitespace is tolerated to match the loader's `Number(" 5 ") == 5`.
    assert_eq!(
      wasm_async_work_pool_size(Some(" 5 ".to_string()), None),
      5,
      "a decimal value with surrounding whitespace must size the pool, matching Number()"
    );
    // Exotic `Number` forms are intentionally NOT mirrored (see the helper doc): the
    // loader's `Number()` would yield 100 / 16 here, but libuv's `atoi` disagrees and
    // nobody sets a pool size this way, so the diagnostics reporter pins them to 4.
    assert_eq!(
      wasm_async_work_pool_size(Some("1e2".to_string()), None),
      4,
      "scientific notation is not mirrored (documented limitation)"
    );
    assert_eq!(
      wasm_async_work_pool_size(Some("0x10".to_string()), None),
      4,
      "hex is not mirrored (documented limitation)"
    );
  }
}
