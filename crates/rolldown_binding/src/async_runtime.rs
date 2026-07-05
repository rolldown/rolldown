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
use napi::bindgen_prelude::{FnArgs, Promise};
use napi_derive::napi;
#[cfg(feature = "async-runtime")]
use rolldown_utils::async_runtime::{
  RuntimeFlavor, RuntimeMetricsSnapshot, RuntimeOptions, TimerDriver, TimerDriverId, TimerId,
  block_on_dyn, configure, configured_options, metrics, register_timer_driver, reset_metrics,
  shutdown, spawn_detached, unregister_timer_driver,
};

use crate::types::js_callback::JsCallback;
#[cfg(feature = "async-runtime")]
use crate::types::js_callback::JsCallbackExt;

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

  fn spawn_blocking(
    &self,
    work: Box<dyn FnOnce() + Send + 'static>,
  ) -> std::result::Result<(), Box<dyn FnOnce() + Send + 'static>> {
    // Keep napi and transitive callers in the same bounded blocking lane as
    // Rolldown's facade. See internal-docs/async-runtime/implementation.md.
    rolldown_utils::async_runtime::spawn_blocking(work).detach();
    Ok(())
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
/// On the `async-runtime` build this reports the runtime controller's
/// validated options: the resolved load-time snapshot (see
/// `resolved_runtime_config`) as clamped by `configure`, including any
/// pre-first-use `configureAsyncRuntime` override. The environment is never
/// re-read.
pub fn get_async_runtime_config() -> BindingRuntimeConfig {
  configured_options().into()
}

// === Unified config-resolution pipeline =====================================
//
// ONE typed resolution replaces the previously divergent per-backend paths:
// every runtime-config environment variable is read in exactly one place
// (`RuntimeEnv::from_process`), resolved through one pure per-(backend,
// target) defaults table (`resolve_runtime_config_for`), and snapshotted once
// per process (`resolved_runtime_config`). Every consumer -- the tokio
// builder in lib.rs `init`, the shared runtime's `register_async_runtime`,
// the `get_async_runtime_config` reporter and the `get_runtime_capabilities`
// export -- reads that same snapshot, so a later `process.env` mutation can
// never make what we report diverge from the runtime that was actually built.
//
// The per-backend DEFAULTS are preserved exactly as measured:
// - tokio-native keeps `physical * 3 / 2` workers and the dedicated
//   4-thread blocking pool (the PR #6270 world);
// - the shared runtime keeps `physical` workers with `max_blocking_tasks`
//   defaulting to the resolved worker count (Task 15 measurement);
// - the threaded-WASI tokio artifact keeps mirroring the napi-rs loader's
//   async work pool size;
// - the shared wasm artifact keeps `RuntimeOptions::default()`'s
//   `available_parallelism` workers (no env worker override, as before).

/// Which scheduler this binding was compiled with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvedRuntimeBackend {
  /// The default `tokio-runtime` build: napi drives a tokio runtime.
  Tokio,
  /// The `--features async-runtime` build: the shared rolldown scheduler.
  Shared,
}

/// Which target family this binding was compiled for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvedRuntimeTarget {
  Native,
  /// `wasm32-wasip1`: threadless wasm (no atomics).
  Wasi,
  /// `wasm32-wasip1-threads`: wasm with real OS threads (atomics).
  WasiThreads,
}

/// Executor flavor, decoupled from the napi types so the pure resolver (and
/// its tests) need no napi machinery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvedRuntimeFlavor {
  CurrentThread,
  MultiThread,
}

impl From<ResolvedRuntimeFlavor> for BindingRuntimeFlavor {
  fn from(value: ResolvedRuntimeFlavor) -> Self {
    match value {
      ResolvedRuntimeFlavor::CurrentThread => Self::CurrentThread,
      ResolvedRuntimeFlavor::MultiThread => Self::MultiThread,
    }
  }
}

#[cfg(feature = "async-runtime")]
impl From<ResolvedRuntimeFlavor> for RuntimeFlavor {
  fn from(value: ResolvedRuntimeFlavor) -> Self {
    match value {
      ResolvedRuntimeFlavor::CurrentThread => Self::CurrentThread,
      ResolvedRuntimeFlavor::MultiThread => Self::MultiThread,
    }
  }
}

/// Raw environment values consumed by the resolver. `from_process` is the
/// ONLY place the process environment is read for runtime configuration.
#[derive(Debug, Clone, Default)]
pub struct RuntimeEnv {
  /// `ROLLDOWN_RUNTIME` -- flavor override; honored by the shared backend
  /// only (silently ignored on tokio builds, as before).
  pub runtime: Option<String>,
  /// `ROLLDOWN_WORKER_THREADS`.
  pub worker_threads: Option<String>,
  /// `ROLLDOWN_MAX_BLOCKING_THREADS`.
  pub max_blocking_threads: Option<String>,
  /// `ROLLDOWN_PARK_DEADLINE_MS` -- opt-in deadline-based `block_on`
  /// deadlock detection; consumed by the shared backend only.
  pub park_deadline_ms: Option<String>,
  /// `NAPI_RS_ASYNC_WORK_POOL_SIZE` -- the napi-rs WASI loader's pool knob,
  /// mirrored by the threaded-WASI tokio reporter arm.
  pub napi_async_work_pool_size: Option<String>,
  /// `UV_THREADPOOL_SIZE` -- the loader's fallback pool knob.
  pub uv_threadpool_size: Option<String>,
}

impl RuntimeEnv {
  /// THE single env-read site for runtime configuration.
  fn from_process() -> Self {
    Self {
      runtime: std::env::var("ROLLDOWN_RUNTIME").ok(),
      worker_threads: std::env::var("ROLLDOWN_WORKER_THREADS").ok(),
      max_blocking_threads: std::env::var("ROLLDOWN_MAX_BLOCKING_THREADS").ok(),
      park_deadline_ms: std::env::var("ROLLDOWN_PARK_DEADLINE_MS").ok(),
      napi_async_work_pool_size: std::env::var("NAPI_RS_ASYNC_WORK_POOL_SIZE").ok(),
      uv_threadpool_size: std::env::var("UV_THREADPOOL_SIZE").ok(),
    }
  }
}

/// The typed result of config resolution: what the runtime is built from
/// (shared backend: PRE-validation values handed to `configure`, which still
/// clamps CurrentThread to one worker) and what the tokio-backend reporter
/// serves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedRuntimeConfig {
  pub backend: ResolvedRuntimeBackend,
  pub target: ResolvedRuntimeTarget,
  pub flavor: ResolvedRuntimeFlavor,
  pub worker_threads: usize,
  pub max_blocking_tasks: usize,
  /// `Some(ms)` only when deadline-based deadlock detection is armed;
  /// resolved for the shared backend only (tokio has no such knob, and the
  /// env var stays ignored there exactly as before).
  pub park_deadline_ms: Option<u64>,
}

const fn compiled_backend() -> ResolvedRuntimeBackend {
  if cfg!(feature = "async-runtime") {
    ResolvedRuntimeBackend::Shared
  } else {
    ResolvedRuntimeBackend::Tokio
  }
}

const fn compiled_target() -> ResolvedRuntimeTarget {
  // `rolldown_wasi_threads` is emitted by build.rs for the exact
  // `wasm32-wasip1-threads` cargo TARGET. It is NOT derivable from built-in
  // cfgs: on current rustc the two WASI targets expose identical cfg sets --
  // `cfg!(target_feature = "atomics")` is false even on the threads target
  // (verified empirically; a threaded artifact built with that predicate
  // reported `target: "wasi"`).
  if cfg!(not(target_family = "wasm")) {
    ResolvedRuntimeTarget::Native
  } else if cfg!(rolldown_wasi_threads) {
    ResolvedRuntimeTarget::WasiThreads
  } else {
    ResolvedRuntimeTarget::Wasi
  }
}

/// Parse a raw `ROLLDOWN_PARK_DEADLINE_MS` value; unset, non-numeric or `0`
/// disable deadline detection rather than erroring (the same lenient
/// treatment `env_config::resolve_thread_count` gives the thread counts --
/// never panic module init over a typo). This is the parse the runtime used
/// to do itself at executor construction
/// (`rolldown_utils::async_runtime::PARK_DEADLINE_ENV`); the read AND the
/// parse now live here, in the single resolver.
fn parse_park_deadline_ms(raw: Option<String>) -> Option<u64> {
  raw.and_then(|value| value.parse::<u64>().ok()).filter(|&millis| millis != 0)
}

/// Parse a raw `ROLLDOWN_RUNTIME` value; unknown / unset values keep
/// `default` (the shared backend's per-target default flavor).
fn resolve_runtime_flavor(
  raw: Option<&str>,
  default: ResolvedRuntimeFlavor,
) -> ResolvedRuntimeFlavor {
  match raw {
    Some("current" | "current-thread" | "single" | "single-thread") => {
      ResolvedRuntimeFlavor::CurrentThread
    }
    Some("multi" | "multi-thread") => ResolvedRuntimeFlavor::MultiThread,
    _ => default,
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
fn wasm_async_work_pool_size(
  napi_async_work_pool_size_env: Option<String>,
  uv_threadpool_size_env: Option<String>,
) -> usize {
  use crate::env_config::resolve_thread_count;
  let selected =
    napi_async_work_pool_size_env.or(uv_threadpool_size_env).map(|value| value.trim().to_string());
  resolve_thread_count(selected, 4)
}

/// The pure per-(backend, target) resolution table. Parameterized on the
/// compile-time facts so every arm is unit-testable on any host; the process
/// entry point is [`resolved_runtime_config`].
fn resolve_runtime_config_for(
  backend: ResolvedRuntimeBackend,
  target: ResolvedRuntimeTarget,
  env: &RuntimeEnv,
) -> ResolvedRuntimeConfig {
  use crate::env_config::resolve_thread_count;
  let native = matches!(target, ResolvedRuntimeTarget::Native);
  match backend {
    ResolvedRuntimeBackend::Tokio => {
      if native {
        // The PR #6270 world: rolldown puts a lot of blocking work on the
        // worker threads rather than the blocking pool, so worker threads are
        // scaled up (physical * 3 / 2) while the blocking pool is pinned to a
        // dedicated 4 (tokio's own default of 512 is far too high for the few
        // genuinely `blocking` tasks rolldown spawns).
        let worker_threads =
          resolve_thread_count(env.worker_threads.clone(), num_cpus::get_physical() * 3 / 2);
        let max_blocking_tasks = resolve_thread_count(env.max_blocking_threads.clone(), 4);
        ResolvedRuntimeConfig {
          backend,
          target,
          flavor: ResolvedRuntimeFlavor::MultiThread,
          worker_threads,
          max_blocking_tasks,
          // tokio has no park-deadline knob; `ROLLDOWN_RUNTIME` and
          // `ROLLDOWN_PARK_DEADLINE_MS` stay silently ignored, as before.
          park_deadline_ms: None,
        }
      } else {
        // Threaded-WASI tokio artifact: no Rust-built runtime exists (lib.rs
        // `init`'s native arm is cfg'd out); the napi-rs WASI loader sizes
        // one async work pool that carries both the worker and the blocking
        // work. Report that single pool honestly for both fields.
        let pool = wasm_async_work_pool_size(
          env.napi_async_work_pool_size.clone(),
          env.uv_threadpool_size.clone(),
        );
        ResolvedRuntimeConfig {
          backend,
          target,
          flavor: ResolvedRuntimeFlavor::MultiThread,
          worker_threads: pool,
          max_blocking_tasks: pool,
          park_deadline_ms: None,
        }
      }
    }
    ResolvedRuntimeBackend::Shared => {
      let default_flavor = if native {
        ResolvedRuntimeFlavor::MultiThread
      } else {
        ResolvedRuntimeFlavor::CurrentThread
      };
      let flavor = resolve_runtime_flavor(env.runtime.as_deref(), default_flavor);
      let worker_threads = if native {
        resolve_thread_count(env.worker_threads.clone(), num_cpus::get_physical())
      } else {
        // `RuntimeOptions::default()` parity: the env worker override has
        // never applied on wasm (`register_async_runtime`'s override block
        // was `not(target_family = "wasm")`), so the default stays
        // `available_parallelism` and `ROLLDOWN_WORKER_THREADS` is ignored.
        std::thread::available_parallelism().map_or(1, usize::from)
      };
      // The blocking default follows the RESOLVED worker count (Task 15
      // measurement), matching the previous `resolve_thread_count(env,
      // options.worker_threads)` ordering.
      let max_blocking_tasks =
        resolve_thread_count(env.max_blocking_threads.clone(), worker_threads);
      ResolvedRuntimeConfig {
        backend,
        target,
        flavor,
        worker_threads,
        max_blocking_tasks,
        park_deadline_ms: parse_park_deadline_ms(env.park_deadline_ms.clone()),
      }
    }
  }
}

/// The per-process resolved runtime-config snapshot. The environment is read
/// exactly once, and lib.rs `init` (a `module_init` hook that runs on EVERY
/// artifact, including the threaded-WASI one that builds no Rust runtime)
/// forces the resolution at module load -- the same moment the WASI loader
/// sizes its async work pool -- so a later env mutation can never make the
/// report diverge from the runtime/pool that already exists, regardless of
/// whether the host's WASI shim snapshots or live-reads its environment.
/// (Electron-reload scope note, unchanged from the previous snapshot: if a
/// host tears the napi env down and recreates it in-process, napi-rs rebuilds
/// its runtime with its own defaults and this snapshot is not refreshed --
/// the fields are diagnostics-only.)
pub fn resolved_runtime_config() -> &'static ResolvedRuntimeConfig {
  static RESOLVED_RUNTIME_CONFIG: std::sync::OnceLock<ResolvedRuntimeConfig> =
    std::sync::OnceLock::new();
  RESOLVED_RUNTIME_CONFIG.get_or_init(|| {
    resolve_runtime_config_for(compiled_backend(), compiled_target(), &RuntimeEnv::from_process())
  })
}

#[cfg(not(feature = "async-runtime"))]
#[napi]
/// Return the effective async runtime configuration.
///
/// Reported from the per-process resolved snapshot: on the native default
/// `tokio-runtime` build these are the thread counts the tokio runtime was
/// ACTUALLY built with at addon load (lib.rs `init` builds from the same
/// snapshot), so a later `process.env` change cannot make the report diverge
/// from the live runtime. On the threaded WASI build it reports the napi-rs
/// WASI loader's async work pool size (NAPI_RS_ASYNC_WORK_POOL_SIZE /
/// UV_THREADPOOL_SIZE), resolved once at first query.
pub fn get_async_runtime_config() -> BindingRuntimeConfig {
  let resolved = resolved_runtime_config();
  BindingRuntimeConfig {
    flavor: resolved.flavor.into(),
    worker_threads: saturating_u32(resolved.worker_threads as u64),
    max_blocking_tasks: saturating_u32(resolved.max_blocking_tasks as u64),
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

/// Host timer driver for the shared runtime's CurrentThread flavor (timer
/// intel §4(b)): `sleep_until` on the single-thread executor cannot park a
/// helper thread (none exists on threadless wasm), so it delegates each timer
/// to the host event loop through the JS callback registered at import --
/// `(ms) => new Promise((resolve) => setTimeout(resolve, ms))`.
///
/// Per timer id: the FIRST `register` arms one host timeout via a detached
/// relay task; re-registers (`Sleep` re-polls) only refresh the stored waker.
/// The resolve-time map removal doubles as the cancellation check -- a
/// `cancel`led (dropped) sleep's entry is gone, so the stale JS timeout fires
/// into nothing (bounded leak: the timeout itself runs out on the JS side,
/// which is at most the debounce delay for the watch coordinator's use).
///
/// LIFETIME (Codex task-7 round 3): each importing napi env registers its own
/// host, and a host dies WITH its env -- the weak threadsafe function does
/// not keep a worker's event loop alive, so a worker that imported the
/// binding can exit at any time and orphan its host. A dead host must never
/// keep timer duty (the registry would busy-fail every debounce against it),
/// so it is EVICTED -- proactively by the env-cleanup hook installed at
/// registration, and reactively by the `is_live` probe (the threadsafe
/// function's `aborted` flag) and by relay-call failure. Eviction wakes every
/// sleep armed here so each re-polls onto the registry's next live registrant
/// (see `TimerDriverRegistry`).
#[cfg(feature = "async-runtime")]
struct JsTimerHost {
  inner: std::sync::Arc<JsTimerHostInner>,
}

/// Consecutive NON-LIFETIME relay failures tolerated on one live host before
/// eviction (Codex task-7 round 4, finding 2). A transient failure (a one-off
/// JS rejection, a queueing hiccup) must not poison a live driver -- on a
/// main-only process that would leave NO driver and every later CT sleep
/// would hit the loud no-driver panic. But a PERSISTENTLY failing live
/// callback can never fire a timer either, so after this many consecutive
/// failures the host is evicted anyway (announced in the log). Reset on any
/// successful relay. Small on purpose: each strike costs one wasted arm/wake
/// round-trip for the affected sleep.
#[cfg(feature = "async-runtime")]
const HOST_TIMER_MAX_TRANSIENT_FAILURES: u32 = 3;

/// Does this relay error mean the HOST IS GONE (evict immediately), as
/// opposed to a callback failure on a live host (strike-counted)?
///
/// NO message strings (Codex task-7 round 5): a rejected JS promise is
/// coerced into `GenericFailure` CARRYING THE JS REJECTION STRING (pinned
/// napi 3.10, error.rs `From<Unknown> for Error`: native coerces the value
/// to a string, wasm reads `.message` -- both always `GenericFailure`), so
/// any message match is forgeable by a live callback rejecting with a
/// colliding string (e.g. `Error('oneshot canceled')`) and would evict a
/// live host, bypassing the strike budget. Two string-free authorities
/// instead:
/// - `Status::Closing` is LIFETIME: it originates only from the TSFN layer
///   (aborted pre-check, raw `napi_closing`), and the JS coercion above can
///   never produce it -- unforgeable.
/// - Everything else defers to the LIVENESS PROBE (`is_live` = the dead
///   latch + the threadsafe function's own `aborted` flag): the genuine
///   teardown shapes that used to be string-matched (queue drained at env
///   teardown, env died before the JS promise settled) all coincide with the
///   env being torn down, which the probe observes directly.
///
/// Race walk: env dies between the error and the probe read -> the probe
/// reads dead -> evict: correct. Env alive at the probe but dying a
/// microsecond later -> strike now (the affected sleep is re-woken); the
/// death is then caught by the env-cleanup hook, by the aborted-probe sweep
/// at the next selection, or by the next relay failure (which will probe
/// dead) -> bounded, correct. A LIVE host's failure -- whatever its message
/// says -- takes the strike path.
#[cfg(feature = "async-runtime")]
fn should_evict_for_relay_error(status: napi::Status, host_is_live: bool) -> bool {
  status == napi::Status::Closing || !host_is_live
}

#[cfg(feature = "async-runtime")]
struct JsTimerHostInner {
  callback: JsCallback<FnArgs<(f64,)>, Promise<()>>,
  pending: std::sync::Mutex<rustc_hash::FxHashMap<TimerId, std::task::Waker>>,
  /// Latched by [`JsTimerHostInner::evict`]: this host's env is gone (or its
  /// callback failed) and it must never serve another timer.
  dead: std::sync::atomic::AtomicBool,
  /// This host's registration in the global driver registry, taken (exactly
  /// once) by `evict`.
  registration: std::sync::Mutex<Option<TimerDriverId>>,
  /// Consecutive non-lifetime relay failures (see
  /// [`HOST_TIMER_MAX_TRANSIENT_FAILURES`]); reset on success.
  transient_failures: std::sync::atomic::AtomicU32,
}

#[cfg(feature = "async-runtime")]
impl JsTimerHostInner {
  fn lock_pending(
    &self,
  ) -> std::sync::MutexGuard<'_, rustc_hash::FxHashMap<TimerId, std::task::Waker>> {
    self.pending.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
  }

  /// Can this host still deliver wakes? The `aborted` probe reads the
  /// threadsafe function's own closing flag, so a dying env is detected even
  /// before any eviction path ran.
  fn is_live(&self) -> bool {
    !self.dead.load(std::sync::atomic::Ordering::SeqCst) && !self.callback.aborted()
  }

  /// Remove this host from timer duty: latch `dead`, drop the registry
  /// entry, and wake every sleep armed here so each re-polls onto the next
  /// live registrant (absolute deadlines preserve the remaining time; with no
  /// live registrant left the re-poll fails LOUD in `rolldown_utils`).
  /// Idempotent -- the cleanup hook, the `is_live` race path, and the
  /// relay-failure backstop may all reach it.
  fn evict(&self) {
    self.dead.store(true, std::sync::atomic::Ordering::SeqCst);
    let registration =
      self.registration.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take();
    if let Some(id) = registration {
      unregister_timer_driver(id);
    }
    let wakers: Vec<std::task::Waker> =
      self.lock_pending().drain().map(|(_, waker)| waker).collect();
    for waker in wakers {
      waker.wake();
    }
  }
}

#[cfg(feature = "async-runtime")]
impl TimerDriver for JsTimerHost {
  fn register(&self, id: TimerId, deadline: std::time::Instant, waker: std::task::Waker) {
    if !self.inner.is_live() {
      // The selecting poll raced this host's death: make sure the eviction
      // bookkeeping ran, then wake immediately so the sleep re-selects from
      // the registry (which no longer offers this host).
      self.inner.evict();
      waker.wake();
      return;
    }
    {
      let mut pending = self.inner.lock_pending();
      if let Some(slot) = pending.get_mut(&id) {
        // Re-poll of a still-armed sleep: refresh the waker only.
        *slot = waker;
        return;
      }
      pending.insert(id, waker);
    }
    let ms = deadline.saturating_duration_since(std::time::Instant::now()).as_secs_f64() * 1000.0;
    let inner = std::sync::Arc::clone(&self.inner);
    spawn_detached(async move {
      let result = async { inner.callback.invoke_async(FnArgs { data: (ms,) }).await?.await }.await;
      match result {
        Ok(()) => {
          inner.transient_failures.store(0, std::sync::atomic::Ordering::SeqCst);
          let woken = inner.lock_pending().remove(&id);
          if let Some(waker) = woken {
            waker.wake();
          }
        }
        // A dead env surfaces here as an error, never a silent hang. The
        // classification is string-free (see `should_evict_for_relay_error`):
        // the unforgeable `Closing` status or the liveness probe reading dead
        // AT THIS MOMENT -- probing after the error keeps the race bounded
        // (an env dying right after a live probe is caught by the cleanup
        // hook, the sweep, or the next relay failure). The host is gone:
        // evict it -- which wakes everything armed here, this sleep included,
        // so each re-polls onto the next live registrant -- instead of waking
        // into a busy retry loop against the corpse.
        Err(error) if should_evict_for_relay_error(error.status, inner.is_live()) => {
          eprintln!("rolldown: host timer callback failed (host gone, evicting): {error}");
          inner.evict();
        }
        // A failure on a provably LIVE host (a JS throw or rejection --
        // regardless of what its message says -- or a wrong return type): do
        // not evict for a transient hiccup; that would strand a main-only
        // process driverless and turn later CT sleeps into loud no-driver
        // panics. Wake just this sleep so it re-polls (and re-arms with its
        // remaining time); a persistently failing callback exhausts the
        // strike budget and is then evicted.
        Err(error) => {
          let strikes =
            inner.transient_failures.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
          if strikes >= HOST_TIMER_MAX_TRANSIENT_FAILURES {
            eprintln!(
              "rolldown: host timer callback failed {strikes} times in a row, evicting this \
               timer host: {error}"
            );
            inner.evict();
          } else {
            eprintln!(
              "rolldown: host timer callback failed \
               ({strikes}/{HOST_TIMER_MAX_TRANSIENT_FAILURES} before eviction): {error}"
            );
            let woken = inner.lock_pending().remove(&id);
            if let Some(waker) = woken {
              waker.wake();
            }
          }
        }
      }
    });
  }

  fn cancel(&self, id: TimerId) {
    let _ = self.inner.lock_pending().remove(&id);
  }

  fn is_live(&self) -> bool {
    self.inner.is_live()
  }

  fn on_swept(&self) {
    // The registry's selection sweep noticed this host's death (the
    // `aborted` probe can fire before the env-cleanup hook runs): run the
    // full eviction so every sleep pending here is woken into re-selection
    // instead of stranded (Codex task-7 round 4, finding 1). Idempotent with
    // the hook and the relay backstop.
    self.inner.evict();
  }
}

#[cfg(feature = "async-runtime")]
#[napi(ts_args_type = "callback: (ms: number) => Promise<void>")]
/// Install the host timer callback backing the shared async runtime's
/// CurrentThread timers (watch-mode debounce). Called at import by every
/// binding-loading JS entry with
/// `(ms) => new Promise((resolve) => setTimeout(resolve, ms))`; each
/// importing env (main thread and workers alike) registers its own host, and
/// the newest live one serves. A no-op on the default `tokio-runtime` build
/// (tokio owns its timer wheel).
pub fn register_timer_host(
  env: &napi::Env,
  callback: JsCallback<FnArgs<(f64,)>, Promise<()>>,
) -> napi::Result<()> {
  let inner = std::sync::Arc::new(JsTimerHostInner {
    callback,
    pending: std::sync::Mutex::default(),
    dead: std::sync::atomic::AtomicBool::new(false),
    registration: std::sync::Mutex::default(),
    transient_failures: std::sync::atomic::AtomicU32::new(0),
  });
  {
    // Hold the registration slot across the registry insert so a
    // concurrently running `evict` (impossible this early in practice, but
    // free to order correctly) can never observe the id half-stored.
    let mut slot = inner.registration.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    *slot = Some(register_timer_driver(std::sync::Arc::new(JsTimerHost {
      inner: std::sync::Arc::clone(&inner),
    })));
  }
  // Proactive eviction at env teardown (worker exit): the primary lifetime
  // mechanism; the `aborted` probe and the relay-failure path in the driver
  // are the backstops for anything the hook cannot reach in time.
  let hook_inner = std::sync::Arc::clone(&inner);
  env.add_env_cleanup_hook(hook_inner, |inner| {
    inner.evict();
  })?;
  Ok(())
}

#[cfg(not(feature = "async-runtime"))]
#[napi(ts_args_type = "callback: (ms: number) => Promise<void>")]
/// Install the host timer callback backing the shared async runtime's
/// CurrentThread timers (watch-mode debounce). Called at import by every
/// binding-loading JS entry with
/// `(ms) => new Promise((resolve) => setTimeout(resolve, ms))`; each
/// importing env (main thread and workers alike) registers its own host, and
/// the newest live one serves. A no-op on the default `tokio-runtime` build
/// (tokio owns its timer wheel).
pub fn register_timer_host(callback: JsCallback<FnArgs<(f64,)>, Promise<()>>) {
  let _ = callback;
}

#[cfg(feature = "async-runtime")]
#[napi_derive::module_init]
fn register_async_runtime() {
  // Consume the SAME resolved snapshot the reporter and the capability export
  // read (the single config-resolution pipeline); `configure` still validates
  // and clamps (CurrentThread => one worker), and the runtime controller's
  // validated options remain the reporting authority on this build.
  let resolved = resolved_runtime_config();
  let options = RuntimeOptions {
    flavor: resolved.flavor.into(),
    worker_threads: resolved.worker_threads,
    max_blocking_tasks: resolved.max_blocking_tasks,
    // Resolved from `ROLLDOWN_PARK_DEADLINE_MS` by the single resolver; the
    // runtime itself no longer reads the environment at executor construction.
    park_deadline: resolved.park_deadline_ms.map(std::time::Duration::from_millis),
    ..RuntimeOptions::default()
  };
  configure(options).expect("Failed to configure the Rolldown async runtime");
  create_custom_async_runtime(RolldownAsyncRuntime);
}

/// What this Rolldown binding IS -- backend, flavor, target -- and the
/// capabilities that follow from it. Values are compile-time facts plus the
/// resolved runtime snapshot; nothing re-reads the environment. Tests and
/// embedders query the artifact instead of inferring the build flavor from
/// env vars or error-message probes.
// The bools ARE the contract: independent capability flags on a napi object
// consumed from JS, not state to be modeled as an enum.
#[expect(clippy::struct_excessive_bools)]
#[napi(object)]
pub struct BindingRuntimeCapabilities {
  /// The scheduler the binding was compiled with: 'tokio' (the default
  /// build) or 'shared' (`--features async-runtime`).
  #[napi(ts_type = "'tokio' | 'shared'")]
  pub backend: String,
  /// The executor flavor actually in effect (post-validation; on the shared
  /// build this reflects a pre-first-use `configureAsyncRuntime` override).
  pub flavor: BindingRuntimeFlavor,
  /// The compile target: 'native', 'wasi' (threadless `wasm32-wasip1`) or
  /// 'wasi-threads' (`wasm32-wasip1-threads`).
  #[napi(ts_type = "'native' | 'wasi' | 'wasi-threads'")]
  pub target: String,
  /// Convenience: the binding is a WebAssembly/WASI artifact (`target !==
  /// 'native'`).
  pub wasi: bool,
  /// Compiled with `--features async-runtime` (either flavor).
  pub async_runtime_build: bool,
  /// Work is scheduled across multiple threads (`flavor === 'MultiThread'`).
  pub threads: bool,
  /// A timer facility backs `sleep_until` (the watch-mode debounce). This is
  /// LIVE HOST-REGISTRATION STATE, the one live field: always true on tokio
  /// builds (tokio owns a timer wheel) and on the shared MultiThread flavor
  /// (executor-owned timer heap); on the shared CurrentThread flavor timers
  /// are delegated to the host event loop, so this reads true while a LIVE
  /// `registerTimerHost` registrant exists. Every public package entry that
  /// loads the binding registers a host driver per importing env at import,
  /// so through any supported entry the answer is true; a registrant whose
  /// env died (an exited worker) is evicted and does NOT count. Only a raw
  /// binding loaded outside the supported entries can observe false (a
  /// CurrentThread `sleep_until` would panic at that point).
  pub timers: bool,
  /// Watch mode is supported by THIS ARTIFACT: static per artifact, true on
  /// both native flavors, false on every wasm artifact (watch on WASI stalls
  /// on the initial build). Deliberately independent of the live `timers`
  /// registration state -- it describes what the artifact can do, and every
  /// public entry registers the timer host the watch debounce needs before
  /// exposing any API.
  pub watch_supported: bool,
  /// `block_on` over a JS-thread continuation cannot deadlock the calling
  /// thread. False on the CurrentThread flavor (the block_on-over-JS
  /// hazard: parking the only thread starves the JS continuation forever).
  pub block_on_js_thread_safe: bool,
}

#[napi]
/// Report the loaded binding's runtime capabilities (see
/// `BindingRuntimeCapabilities`). Derived from compile-time cfg plus the
/// resolved runtime snapshot -- never from re-reading the environment.
pub fn get_runtime_capabilities() -> BindingRuntimeCapabilities {
  let resolved = resolved_runtime_config();
  let target = match resolved.target {
    ResolvedRuntimeTarget::Native => "native",
    ResolvedRuntimeTarget::Wasi => "wasi",
    ResolvedRuntimeTarget::WasiThreads => "wasi-threads",
  };
  let wasi = !matches!(resolved.target, ResolvedRuntimeTarget::Native);
  let async_runtime_build = matches!(resolved.backend, ResolvedRuntimeBackend::Shared);

  #[cfg(feature = "async-runtime")]
  let (flavor, timers) = {
    // The runtime controller's validated options are the flavor authority on
    // this build: they include a pre-first-use `configureAsyncRuntime`
    // override, which the load-time snapshot cannot know about.
    let flavor: BindingRuntimeFlavor = configured_options().flavor.into();
    let timers = match flavor {
      // Executor-owned timer heap, available unconditionally.
      BindingRuntimeFlavor::MultiThread => true,
      // Host-delegated timers: available while a LIVE driver is registered
      // (see the `timers` field doc for the before-registration case). Dead
      // registrants -- hosts whose envs were torn down -- do not count, so
      // this cannot read `true` off a worker-registered driver that died
      // with its worker.
      BindingRuntimeFlavor::CurrentThread => rolldown_utils::async_runtime::has_live_timer_driver(),
    };
    (flavor, timers)
  };
  #[cfg(not(feature = "async-runtime"))]
  let (flavor, timers) = {
    // tokio owns a timer wheel on every tokio-runtime artifact (the native
    // build and the napi-built runtime on threaded WASI alike).
    (BindingRuntimeFlavor::from(resolved.flavor), true)
  };

  let threads = matches!(flavor, BindingRuntimeFlavor::MultiThread);
  BindingRuntimeCapabilities {
    backend: if async_runtime_build { "shared" } else { "tokio" }.to_string(),
    flavor,
    target: target.to_string(),
    wasi,
    async_runtime_build,
    threads,
    timers,
    // Static per artifact (see the field doc): the capability contract must
    // not depend on import order or registration state.
    watch_supported: !wasi,
    block_on_js_thread_safe: threads,
  }
}

// Resolver tests are parameterized on (backend, target), so every arm of the
// defaults table is exercised under BOTH feature profiles and on any host.
// The snapshot-wins-over-later-env property that the old reporter test pinned
// is now structural: the environment is read exactly once, inside the
// `OnceLock` initializer of `resolved_runtime_config`.
#[cfg(test)]
mod tests {
  use super::{
    ResolvedRuntimeBackend, ResolvedRuntimeFlavor, ResolvedRuntimeTarget, RuntimeEnv,
    parse_park_deadline_ms, resolve_runtime_config_for, wasm_async_work_pool_size,
  };

  fn env() -> RuntimeEnv {
    RuntimeEnv::default()
  }

  fn resolve(
    backend: ResolvedRuntimeBackend,
    target: ResolvedRuntimeTarget,
    env: &RuntimeEnv,
  ) -> super::ResolvedRuntimeConfig {
    resolve_runtime_config_for(backend, target, env)
  }

  #[cfg(feature = "async-runtime")]
  #[test]
  fn napi_spawn_blocking_routes_through_the_shared_runtime() {
    rolldown_utils::async_runtime::reset_metrics();
    let handle =
      napi::bindgen_prelude::spawn_blocking(|| std::thread::current().name().map(str::to_owned));
    let thread_name = futures::executor::block_on(handle)
      .expect("the shared blocking lane should execute napi work");

    assert_ne!(
      thread_name.as_deref(),
      Some(napi::bindgen_prelude::SPAWN_BLOCKING_FALLBACK_THREAD_NAME),
      "Rolldown's hook must accept the work instead of using napi's fallback thread"
    );
    assert!(
      rolldown_utils::async_runtime::metrics().blocking_tasks_started >= 1,
      "napi work should be recorded by the shared blocking scheduler"
    );
  }

  #[test]
  fn tokio_native_defaults_are_the_measured_pr6270_world() {
    let resolved = resolve(ResolvedRuntimeBackend::Tokio, ResolvedRuntimeTarget::Native, &env());
    assert_eq!(resolved.flavor, ResolvedRuntimeFlavor::MultiThread);
    assert_eq!(
      resolved.worker_threads,
      num_cpus::get_physical() * 3 / 2,
      "tokio-native scales workers to physical * 3 / 2"
    );
    assert_eq!(resolved.max_blocking_tasks, 4, "tokio-native keeps the dedicated 4-thread pool");
    assert_eq!(resolved.park_deadline_ms, None);
  }

  #[test]
  fn tokio_native_honors_thread_env_overrides_and_ignores_the_rest() {
    let resolved = resolve(
      ResolvedRuntimeBackend::Tokio,
      ResolvedRuntimeTarget::Native,
      &RuntimeEnv {
        runtime: Some("single".to_string()),
        worker_threads: Some("7".to_string()),
        max_blocking_threads: Some("5".to_string()),
        park_deadline_ms: Some("1500".to_string()),
        ..RuntimeEnv::default()
      },
    );
    assert_eq!((resolved.worker_threads, resolved.max_blocking_tasks), (7, 5));
    // Pins today's behavior: the tokio backend silently ignores
    // ROLLDOWN_RUNTIME and ROLLDOWN_PARK_DEADLINE_MS.
    assert_eq!(resolved.flavor, ResolvedRuntimeFlavor::MultiThread);
    assert_eq!(resolved.park_deadline_ms, None);
    // Unusable values (zero / garbage) fall back to the defaults, matching
    // `resolve_thread_count` -- a typo must never panic module init.
    let fallback = resolve(
      ResolvedRuntimeBackend::Tokio,
      ResolvedRuntimeTarget::Native,
      &RuntimeEnv {
        worker_threads: Some("0".to_string()),
        max_blocking_threads: Some("abc".to_string()),
        ..RuntimeEnv::default()
      },
    );
    assert_eq!(fallback.worker_threads, num_cpus::get_physical() * 3 / 2);
    assert_eq!(fallback.max_blocking_tasks, 4);
  }

  #[test]
  fn tokio_wasi_threads_mirrors_the_loader_pool_for_both_fields() {
    // Finding B lineage: the threaded-WASI arm must size the pool from the
    // SAME env keys and precedence the napi-rs WASI loader uses.
    let resolved = resolve(
      ResolvedRuntimeBackend::Tokio,
      ResolvedRuntimeTarget::WasiThreads,
      &RuntimeEnv {
        napi_async_work_pool_size: Some("6".to_string()),
        uv_threadpool_size: Some("2".to_string()),
        // Ignored on this arm: the loader, not rolldown, owns the pool.
        worker_threads: Some("99".to_string()),
        max_blocking_threads: Some("98".to_string()),
        ..RuntimeEnv::default()
      },
    );
    assert_eq!(resolved.flavor, ResolvedRuntimeFlavor::MultiThread);
    assert_eq!(
      (resolved.worker_threads, resolved.max_blocking_tasks),
      (6, 6),
      "one loader pool carries both the worker and the blocking work"
    );
  }

  #[test]
  fn wasm_pool_size_matches_loader_env_precedence() {
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

  #[test]
  fn shared_native_defaults_follow_the_task15_measurement() {
    let resolved = resolve(ResolvedRuntimeBackend::Shared, ResolvedRuntimeTarget::Native, &env());
    assert_eq!(resolved.flavor, ResolvedRuntimeFlavor::MultiThread);
    assert_eq!(resolved.worker_threads, num_cpus::get_physical());
    assert_eq!(
      resolved.max_blocking_tasks, resolved.worker_threads,
      "the shared blocking default is the resolved worker count"
    );
    assert_eq!(resolved.park_deadline_ms, None);
  }

  #[test]
  fn shared_native_env_overrides_and_flavor_selection() {
    // The blocking default follows the RESOLVED worker count, not the cpu
    // default: with workers overridden to 7 and no blocking override, the
    // blocking cap is 7.
    let resolved = resolve(
      ResolvedRuntimeBackend::Shared,
      ResolvedRuntimeTarget::Native,
      &RuntimeEnv { worker_threads: Some("7".to_string()), ..RuntimeEnv::default() },
    );
    assert_eq!((resolved.worker_threads, resolved.max_blocking_tasks), (7, 7));

    for (raw, expected) in [
      ("single", ResolvedRuntimeFlavor::CurrentThread),
      ("single-thread", ResolvedRuntimeFlavor::CurrentThread),
      ("current", ResolvedRuntimeFlavor::CurrentThread),
      ("current-thread", ResolvedRuntimeFlavor::CurrentThread),
      ("multi", ResolvedRuntimeFlavor::MultiThread),
      ("multi-thread", ResolvedRuntimeFlavor::MultiThread),
      // Unknown values keep the per-target default (MultiThread on native).
      ("turbo", ResolvedRuntimeFlavor::MultiThread),
    ] {
      let resolved = resolve(
        ResolvedRuntimeBackend::Shared,
        ResolvedRuntimeTarget::Native,
        &RuntimeEnv { runtime: Some(raw.to_string()), ..RuntimeEnv::default() },
      );
      assert_eq!(resolved.flavor, expected, "ROLLDOWN_RUNTIME={raw}");
    }
  }

  #[test]
  fn shared_park_deadline_parsing_treats_unset_zero_and_garbage_as_disabled() {
    // Ports the parse test from rolldown_utils (the read AND the parse moved
    // here): never panic module init over a typo, just disable detection.
    assert_eq!(parse_park_deadline_ms(None), None);
    assert_eq!(parse_park_deadline_ms(Some("0".to_string())), None);
    assert_eq!(parse_park_deadline_ms(Some("abc".to_string())), None);
    assert_eq!(parse_park_deadline_ms(Some("-5".to_string())), None);
    assert_eq!(parse_park_deadline_ms(Some(String::new())), None);
    assert_eq!(parse_park_deadline_ms(Some("1500".to_string())), Some(1500));

    // And the resolver wires it through on the shared backend.
    let resolved = resolve(
      ResolvedRuntimeBackend::Shared,
      ResolvedRuntimeTarget::Native,
      &RuntimeEnv { park_deadline_ms: Some("60000".to_string()), ..RuntimeEnv::default() },
    );
    assert_eq!(resolved.park_deadline_ms, Some(60000));
  }

  #[test]
  fn shared_wasi_defaults_keep_runtime_options_parity() {
    let host_parallelism = std::thread::available_parallelism().map_or(1, usize::from);
    for target in [ResolvedRuntimeTarget::Wasi, ResolvedRuntimeTarget::WasiThreads] {
      let resolved = resolve(ResolvedRuntimeBackend::Shared, target, &env());
      assert_eq!(
        resolved.flavor,
        ResolvedRuntimeFlavor::CurrentThread,
        "the shared wasm default flavor is CurrentThread"
      );
      assert_eq!(resolved.worker_threads, host_parallelism);
      assert_eq!(resolved.max_blocking_tasks, resolved.worker_threads);

      // ROLLDOWN_WORKER_THREADS has never applied on wasm; the blocking
      // override and the flavor selection do. (Selecting `multi` here is
      // still resolved raw -- `configure`'s validation is what rejects it on
      // a threadless build, exactly as before.)
      let overridden = resolve(
        ResolvedRuntimeBackend::Shared,
        target,
        &RuntimeEnv {
          runtime: Some("multi".to_string()),
          worker_threads: Some("9".to_string()),
          max_blocking_threads: Some("3".to_string()),
          ..RuntimeEnv::default()
        },
      );
      assert_eq!(
        overridden.worker_threads, host_parallelism,
        "the env worker override must stay ignored on wasm"
      );
      assert_eq!(overridden.max_blocking_tasks, 3);
      assert_eq!(overridden.flavor, ResolvedRuntimeFlavor::MultiThread);
    }
  }

  /// Codex task-7 rounds 4+5: the relay must evict ONLY on host death, and
  /// the decision must be STRING-FREE -- a rejected JS promise coerces to
  /// `GenericFailure` carrying the JS-controlled rejection string (pinned
  /// napi 3.10 error.rs), so message matching is forgeable by a live
  /// callback. The two authorities: the unforgeable `Closing` status, and
  /// the liveness probe.
  #[cfg(feature = "async-runtime")]
  #[test]
  fn relay_eviction_is_decided_by_status_and_probe_never_by_message() {
    use napi::Status;

    use super::should_evict_for_relay_error;

    // `Closing` is authoritative host death: it originates only from the
    // TSFN layer (aborted pre-check, raw napi_closing) and JS coercion can
    // never produce it -- evict even if the probe still reads live (the
    // finalize flag can lag the raw status).
    assert!(should_evict_for_relay_error(Status::Closing, true), "Closing overrides a live probe");
    assert!(should_evict_for_relay_error(Status::Closing, false));

    // A DEAD probe evicts regardless of status -- this covers the genuine
    // teardown shapes that used to be string-matched (queue drained at env
    // teardown, env died before the JS promise settled): both coincide with
    // the env being torn down, which the probe observes directly.
    assert!(should_evict_for_relay_error(Status::GenericFailure, false));
    assert!(should_evict_for_relay_error(Status::PendingException, false));

    // Codex round-5 regression: a LIVE host's failure takes the strike path
    // no matter what the error says -- including a JS rejection whose
    // message collides with napi's internal teardown strings. (The RED
    // JS-level shape: `Promise.reject(new Error('oneshot canceled'))` from a
    // live callback coerces to GenericFailure + "Error: oneshot canceled"
    // and used to substring-match the old classifier into evicting.)
    assert!(
      !should_evict_for_relay_error(Status::GenericFailure, true),
      "a live host's GenericFailure -- e.g. a forged 'oneshot canceled' rejection -- must strike"
    );
    assert!(
      !should_evict_for_relay_error(Status::PendingException, true),
      "a JS throw on a live host must strike"
    );
    assert!(
      !should_evict_for_relay_error(Status::InvalidArg, true),
      "a wrong return type on a live host must strike"
    );
    assert!(
      !should_evict_for_relay_error(Status::QueueFull, true),
      "queue pressure on a live host must strike"
    );
  }
}
