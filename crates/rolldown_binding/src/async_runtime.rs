// The public napi surface in this module (the `Binding*` `#[napi(object)]` types and
// the `configure_async_runtime` / `get_async_runtime_*` / `reset_async_runtime_metrics`
// `#[napi]` exports) is reachable only from JS. The in-crate unit-test binary never
// constructs or calls it, so `dead_code` flags it in the TEST profile. Relax dead_code
// for the TEST profile only: genuinely dead library code is still caught by the
// non-test (cdylib) clippy gate, which carries no such allow.
#![cfg_attr(test, allow(dead_code))]

use std::{future::Future, pin::Pin, ptr};

use napi::bindgen_prelude::{
  AsyncRuntime, AsyncRuntimeRejection, AsyncRuntimeTask, register_async_runtime,
};
use napi::bindgen_prelude::{FnArgs, Promise, Unknown};
use napi::threadsafe_function::ThreadsafeFunctionCallMode;
use napi_derive::napi;
use rolldown_utils::MAX_ASYNC_RUNTIME_WORKER_THREADS;
use rolldown_utils::async_runtime::{
  CurrentThreadTaskDelivery, CurrentThreadTaskDriver, CurrentThreadTaskDriverId, RuntimeFlavor,
  RuntimeMetricsSnapshot, RuntimeOptions, RuntimeOptionsPatch, TimerDriver, TimerDriverId, TimerId,
  acknowledge_current_thread_task_delivery, configure, configure_partial, configured_options,
  drive_current_thread_tasks, fail_current_thread_task_delivery, metrics,
  register_current_thread_task_driver, register_timer_driver, request_current_thread_task_drain,
  reset_metrics, shutdown, start, try_block_on_dyn, try_spawn, try_spawn_blocking,
  try_spawn_detached, unregister_current_thread_task_driver, unregister_timer_driver,
};
use rolldown_utils::max_async_runtime_worker_threads;

use crate::types::js_callback::InvalidReturnValue;
use crate::types::js_callback::JsCallback;

struct RolldownAsyncRuntime;

// SAFETY: Shutdown closes
// admission, waits for the scheduler generation to quiesce, joins native
// workers, and releases active resources. Independently, napi-rs permanently
// retains the native image after a module that registered this backend exports
// successfully, so externally cloned wakers cannot call into unmapped code.
unsafe impl AsyncRuntime for RolldownAsyncRuntime {
  fn spawn(
    &self,
    task: AsyncRuntimeTask,
  ) -> std::result::Result<(), AsyncRuntimeRejection<AsyncRuntimeTask>> {
    match try_spawn(task) {
      Ok(handle) => {
        handle.detach();
        Ok(())
      }
      Err((error, task)) => {
        Err(AsyncRuntimeRejection::new(task, napi::Error::from_reason(error.to_string())))
      }
    }
  }

  fn block_on(&self, future: Pin<&mut dyn Future<Output = ()>>) -> napi::Result<()> {
    try_block_on_dyn(future).map_err(|error| napi::Error::from_reason(error.to_string()))
  }

  fn spawn_blocking(
    &self,
    work: Box<dyn FnOnce() + Send + 'static>,
  ) -> std::result::Result<(), AsyncRuntimeRejection<Box<dyn FnOnce() + Send + 'static>>> {
    // Route blocking work submitted through this SPI to the same bounded lane
    // as Rolldown's facade.
    match try_spawn_blocking(work) {
      Ok(handle) => {
        handle.detach();
        Ok(())
      }
      Err((error, work)) => {
        Err(AsyncRuntimeRejection::new(work, napi::Error::from_reason(error.to_string())))
      }
    }
  }

  fn start(&self) -> napi::Result<()> {
    start().map_err(|error| napi::Error::from_reason(error.to_string()))
  }

  fn shutdown(&self) -> napi::Result<()> {
    shutdown().map_err(|error| napi::Error::from_reason(error.to_string()))
  }
}

#[napi(string_enum)]
#[derive(Clone, Copy)]
pub enum BindingRuntimeFlavor {
  CurrentThread,
  MultiThread,
}

impl From<BindingRuntimeFlavor> for RuntimeFlavor {
  fn from(value: BindingRuntimeFlavor) -> Self {
    match value {
      BindingRuntimeFlavor::CurrentThread => Self::CurrentThread,
      BindingRuntimeFlavor::MultiThread => Self::MultiThread,
    }
  }
}

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
  /// Positive integer worker count. Values above 256 are rejected.
  pub worker_threads: Option<f64>,
  /// Positive integer blocking-task limit. Values above 256 are rejected.
  pub max_blocking_tasks: Option<f64>,
}

impl TryFrom<BindingRuntimeOptions> for RuntimeOptionsPatch {
  type Error = napi::Error;

  fn try_from(value: BindingRuntimeOptions) -> Result<Self, Self::Error> {
    Ok(Self {
      flavor: value.flavor.map(Into::into),
      worker_threads: validate_binding_thread_count(
        "workerThreads",
        value.worker_threads,
        MAX_ASYNC_RUNTIME_WORKER_THREADS,
      )?,
      max_blocking_tasks: validate_binding_thread_count(
        "maxBlockingTasks",
        value.max_blocking_tasks,
        MAX_ASYNC_RUNTIME_WORKER_THREADS,
      )?,
    })
  }
}

fn validate_binding_thread_count(
  field: &str,
  value: Option<f64>,
  maximum: usize,
) -> napi::Result<Option<usize>> {
  value
    .map(|count| {
      #[expect(
        clippy::cast_precision_loss,
        reason = "the small production thread limit is exactly representable as f64"
      )]
      let maximum_as_f64 = maximum as f64;
      if !count.is_finite() || count < 1.0 || count.fract() != 0.0 || count > maximum_as_f64 {
        return Err(napi::Error::from_reason(format!(
          "`{field}` must be a positive integer no greater than {maximum}"
        )));
      }

      #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the finite positive integer was range-checked against the usize-backed limit"
      )]
      let count = count as usize;
      Ok(count)
    })
    .transpose()
}

#[napi(object)]
pub struct BindingRuntimeConfig {
  pub flavor: BindingRuntimeFlavor,
  pub worker_threads: u32,
  pub max_blocking_tasks: u32,
}

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
  pub tasks_spawned: f64,
  pub tasks_completed: f64,
  pub tasks_panicked: f64,
  pub runnable_schedules: f64,
  pub runnable_polls: f64,
  pub queued_runnables: f64,
  pub max_queued_runnables: f64,
  pub active_runnables: f64,
  pub max_active_runnables: f64,
  pub blocking_tasks_started: f64,
  pub blocking_tasks_completed: f64,
  pub active_blocking_tasks: f64,
  pub max_active_blocking_tasks: f64,
}

impl From<RuntimeMetricsSnapshot> for BindingRuntimeMetrics {
  fn from(value: RuntimeMetricsSnapshot) -> Self {
    Self {
      flavor: value.flavor.into(),
      worker_threads: saturating_u32(value.worker_threads as u64),
      max_blocking_tasks: saturating_u32(value.max_blocking_tasks as u64),
      tasks_spawned: safe_js_number(value.tasks_spawned),
      tasks_completed: safe_js_number(value.tasks_completed),
      tasks_panicked: safe_js_number(value.tasks_panicked),
      runnable_schedules: safe_js_number(value.runnable_schedules),
      runnable_polls: safe_js_number(value.runnable_polls),
      queued_runnables: safe_js_number(value.queued_runnables),
      max_queued_runnables: safe_js_number(value.max_queued_runnables),
      active_runnables: safe_js_number(value.active_runnables),
      max_active_runnables: safe_js_number(value.max_active_runnables),
      blocking_tasks_started: safe_js_number(value.blocking_tasks_started),
      blocking_tasks_completed: safe_js_number(value.blocking_tasks_completed),
      active_blocking_tasks: safe_js_number(value.active_blocking_tasks),
      max_active_blocking_tasks: safe_js_number(value.max_active_blocking_tasks),
    }
  }
}

fn saturating_u32(value: u64) -> u32 {
  u32::try_from(value).unwrap_or(u32::MAX)
}

const MAX_SAFE_JS_INTEGER: u64 = (1_u64 << 53) - 1;

#[expect(
  clippy::cast_precision_loss,
  reason = "the value is clamped to JavaScript's exactly representable integer range"
)]
fn safe_js_number(value: u64) -> f64 {
  value.min(MAX_SAFE_JS_INTEGER) as f64
}

#[napi]
/// Override the shared async runtime's flavor and thread counts.
///
/// Must be called before the first async binding call.
pub fn configure_async_runtime(options: BindingRuntimeOptions) -> napi::Result<()> {
  configure_partial(options.try_into()?)
    .map_err(|error| napi::Error::from_reason(error.to_string()))
}

#[napi]
/// Return the effective async runtime configuration.
///
/// Reports the controller's validated options, including a pre-first-use
/// `configureAsyncRuntime` override. The environment is never re-read.
pub fn get_async_runtime_config() -> BindingRuntimeConfig {
  configured_options().into()
}

// === Unified config-resolution pipeline =====================================
//
// ONE typed resolution: every runtime-config environment variable is read in
// exactly one place (`RuntimeEnv::from_process`), resolved through one pure
// per-target defaults table (`resolve_runtime_config_for`), and snapshotted
// once per process (`resolved_runtime_config`). Every consumer -- the shared
// runtime's `register_async_runtime` and the `get_runtime_capabilities`
// export -- reads that same snapshot, so a later `process.env` mutation can
// never make what we report diverge from the runtime that was actually built.
//
// The defaults are preserved within explicit production bounds:
// - the shared native runtime keeps `max(physical, 2)` workers and reserves
//   one execution lane from blocking admission, capped at 256 workers;
// - the wasm artifacts report the CurrentThread executor's one physical
//   execution lane (no env worker override, as before).

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
  /// `ROLLDOWN_RUNTIME` -- flavor override.
  pub runtime: Option<String>,
  /// `ROLLDOWN_WORKER_THREADS`.
  pub worker_threads: Option<String>,
  /// `ROLLDOWN_MAX_BLOCKING_THREADS`.
  pub max_blocking_threads: Option<String>,
  /// `ROLLDOWN_PARK_DEADLINE_MS` -- opt-in deadline-based `block_on`
  /// deadlock detection.
  pub park_deadline_ms: Option<String>,
}

impl RuntimeEnv {
  /// THE single env-read site for runtime configuration.
  fn from_process() -> Self {
    Self {
      runtime: std::env::var("ROLLDOWN_RUNTIME").ok(),
      worker_threads: std::env::var("ROLLDOWN_WORKER_THREADS").ok(),
      max_blocking_threads: std::env::var("ROLLDOWN_MAX_BLOCKING_THREADS").ok(),
      park_deadline_ms: std::env::var("ROLLDOWN_PARK_DEADLINE_MS").ok(),
    }
  }
}

/// The typed result of config resolution: the effective values the runtime is
/// built from. CurrentThread is normalized to one worker; MultiThread is
/// normalized to a truthful minimum of two workers before it reaches the
/// controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedRuntimeConfig {
  pub target: ResolvedRuntimeTarget,
  pub flavor: ResolvedRuntimeFlavor,
  pub worker_threads: usize,
  pub max_blocking_tasks: usize,
  /// `Some(ms)` only when deadline-based deadlock detection is armed.
  pub park_deadline_ms: Option<u64>,
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
/// never panic module init over a typo). The read AND the parse live here,
/// in the single resolver: the shared scheduler never reads the environment
/// itself, and Rolldown deliberately keeps its historical variable name
/// rather than the shared crate's `PARK_DEADLINE_ENV` convention
/// (`NAPI_RUNTIME_PARK_DEADLINE_MS`).
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

fn native_default_parallelism(physical: usize, available: usize) -> usize {
  physical.min(available).max(1)
}

fn detected_native_parallelism() -> usize {
  native_default_parallelism(num_cpus::get_physical(), num_cpus::get())
}

fn clamp_shared_blocking_tasks(
  flavor: ResolvedRuntimeFlavor,
  worker_threads: usize,
  requested: usize,
) -> usize {
  match flavor {
    ResolvedRuntimeFlavor::CurrentThread => 1,
    ResolvedRuntimeFlavor::MultiThread => requested.min(worker_threads.saturating_sub(1).max(1)),
  }
}

/// The pure per-target resolution table. Parameterized on the compile-time
/// facts so every arm is unit-testable on any host; the process entry point
/// is [`resolved_runtime_config`].
fn resolve_runtime_config_for(
  target: ResolvedRuntimeTarget,
  env: &RuntimeEnv,
) -> ResolvedRuntimeConfig {
  use crate::env_config::resolve_thread_count;
  let native = matches!(target, ResolvedRuntimeTarget::Native);
  let default_flavor =
    if native { ResolvedRuntimeFlavor::MultiThread } else { ResolvedRuntimeFlavor::CurrentThread };
  let requested_flavor = resolve_runtime_flavor(env.runtime.as_deref(), default_flavor);
  // The shared scheduler has no MultiThread executor on WebAssembly:
  // `rolldown_utils` does not compile Rayon there. Normalize an unsupported
  // environment override before the module-init hook calls `configure`, so
  // loading a WASI artifact can never panic because `ROLLDOWN_RUNTIME=multi`
  // leaked in from a native process environment.
  let flavor = if native { requested_flavor } else { ResolvedRuntimeFlavor::CurrentThread };
  let requested_worker_threads = if native {
    resolve_thread_count(
      env.worker_threads.clone(),
      detected_native_parallelism(),
      max_async_runtime_worker_threads(),
    )
  } else {
    // `RuntimeOptions::default()` parity: the env worker override has
    // never applied on wasm (`register_async_runtime`'s override block
    // was `not(target_family = "wasm")`), so the default stays
    // `available_parallelism` and `ROLLDOWN_WORKER_THREADS` is ignored.
    std::thread::available_parallelism().map_or(1, usize::from)
  };
  let worker_threads = match flavor {
    ResolvedRuntimeFlavor::CurrentThread => 1,
    ResolvedRuntimeFlavor::MultiThread => requested_worker_threads.max(2),
  };
  let requested_blocking_tasks =
    resolve_thread_count(env.max_blocking_threads.clone(), worker_threads, worker_threads);
  let max_blocking_tasks =
    clamp_shared_blocking_tasks(flavor, worker_threads, requested_blocking_tasks);
  ResolvedRuntimeConfig {
    target,
    flavor,
    worker_threads,
    max_blocking_tasks,
    park_deadline_ms: parse_park_deadline_ms(env.park_deadline_ms.clone()),
  }
}

/// The per-process resolved runtime-config snapshot. The environment is read
/// exactly once, and lib.rs `init` (a `module_init` hook that runs on EVERY
/// artifact) forces the resolution at module load -- the same moment the WASI
/// loader sizes its async work pool -- so a later env mutation can never make
/// the report diverge from the runtime/pool that already exists, regardless
/// of whether the host's WASI shim snapshots or live-reads its environment.
pub fn resolved_runtime_config() -> &'static ResolvedRuntimeConfig {
  static RESOLVED_RUNTIME_CONFIG: std::sync::OnceLock<ResolvedRuntimeConfig> =
    std::sync::OnceLock::new();
  RESOLVED_RUNTIME_CONFIG
    .get_or_init(|| resolve_runtime_config_for(compiled_target(), &RuntimeEnv::from_process()))
}

#[napi]
/// Return a snapshot of the shared async runtime's task and scheduler counters.
pub fn get_async_runtime_metrics() -> BindingRuntimeMetrics {
  metrics().into()
}

#[napi]
/// Reset cumulative async runtime event counters to zero.
///
/// Live gauges and their lifetime high-water marks are preserved so active
/// task guards can complete without corrupting concurrent observations.
pub fn reset_async_runtime_metrics() {
  reset_metrics();
}

#[napi(object)]
pub struct BindingHostRegistration {
  pub high: u32,
  pub low: u32,
}

impl BindingHostRegistration {
  fn from_id(id: u64) -> Self {
    Self {
      high: (id >> 32) as u32,
      low: u32::try_from(id & u64::from(u32::MAX))
        .expect("the masked host registration low word must fit in u32"),
    }
  }

  fn id(high: u32, low: u32) -> u64 {
    (u64::from(high) << 32) | u64::from(low)
  }
}

static NEXT_HOST_REGISTRATION_ID: std::sync::atomic::AtomicU64 =
  std::sync::atomic::AtomicU64::new(1);

static RESERVED_HOST_REGISTRATIONS: std::sync::LazyLock<
  std::sync::Mutex<rustc_hash::FxHashSet<u64>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(rustc_hash::FxHashSet::default()));

fn reserve_host_registration_id() -> napi::Result<u64> {
  let id = NEXT_HOST_REGISTRATION_ID
    .fetch_update(
      std::sync::atomic::Ordering::SeqCst,
      std::sync::atomic::Ordering::SeqCst,
      |current| current.checked_add(1),
    )
    .map_err(|_| {
      napi::Error::new(
        napi::Status::GenericFailure,
        "JavaScript host registration id space exhausted",
      )
    })?;
  RESERVED_HOST_REGISTRATIONS.lock().unwrap_or_else(std::sync::PoisonError::into_inner).insert(id);
  Ok(id)
}

fn claim_host_registration_id(registration_high: u32, registration_low: u32) -> napi::Result<u64> {
  let id = BindingHostRegistration::id(registration_high, registration_low);
  if RESERVED_HOST_REGISTRATIONS
    .lock()
    .unwrap_or_else(std::sync::PoisonError::into_inner)
    .remove(&id)
  {
    Ok(id)
  } else {
    Err(napi::Error::new(
      napi::Status::InvalidArg,
      "CurrentThread host registration was not reserved or was already consumed",
    ))
  }
}

fn release_host_registration_id(id: u64) {
  RESERVED_HOST_REGISTRATIONS.lock().unwrap_or_else(std::sync::PoisonError::into_inner).remove(&id);
}

#[napi]
/// Reserve an exact CurrentThread host registration capability before either
/// task or timer installation performs side effects. The JavaScript package
/// validates the returned words and passes them back to one registration call.
pub fn reserve_current_thread_host_registration() -> napi::Result<BindingHostRegistration> {
  reserve_host_registration_id().map(BindingHostRegistration::from_id)
}

fn current_thread_task_host_napi_result(
  status: napi::sys::napi_status,
  context: &'static str,
) -> napi::Result<()> {
  if status == napi::sys::Status::napi_ok {
    Ok(())
  } else {
    Err(napi::Error::new(napi::Status::from(status), context))
  }
}

fn contain_current_thread_task_host_unwind<T>(operation: impl FnOnce() -> T) -> Option<T> {
  match std::panic::catch_unwind(std::panic::AssertUnwindSafe(operation)) {
    Ok(value) => Some(value),
    Err(payload) => {
      if let Err(nested) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(payload)))
      {
        std::mem::forget(nested);
      }
      None
    }
  }
}

#[cfg(test)]
thread_local! {
  static NATIVE_TASK_HOST_AFTER_DRIVE_TEST_HOOK:
    std::cell::RefCell<Option<Box<dyn FnOnce()>>> =
      const { std::cell::RefCell::new(None) };
  static NATIVE_TASK_HOST_AFTER_PAYLOAD_DROP_TEST_HOOK:
    std::cell::RefCell<Option<Box<dyn FnOnce()>>> =
      const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
fn run_native_task_host_after_drive_test_hook() {
  if let Some(hook) = NATIVE_TASK_HOST_AFTER_DRIVE_TEST_HOOK.with(|slot| slot.borrow_mut().take()) {
    hook();
  }
}

#[cfg(test)]
fn run_native_task_host_after_payload_drop_test_hook() {
  if let Some(hook) =
    NATIVE_TASK_HOST_AFTER_PAYLOAD_DROP_TEST_HOOK.with(|slot| slot.borrow_mut().take())
  {
    hook();
  }
}

unsafe extern "C" fn finalize_native_current_thread_task_host(
  _env: napi::sys::napi_env,
  finalize_data: *mut std::ffi::c_void,
  _finalize_hint: *mut std::ffi::c_void,
) {
  if finalize_data.is_null() {
    return;
  }
  let _ = contain_current_thread_task_host_unwind(|| {
    let weak = unsafe {
      std::sync::Weak::<NativeCurrentThreadTaskHostInner>::from_raw(finalize_data.cast())
    };
    if let Some(inner) = weak.upgrade() {
      inner.finalized();
    }
  });
}

unsafe extern "C" fn call_native_current_thread_task_host(
  env: napi::sys::napi_env,
  _js_callback: napi::sys::napi_value,
  _context: *mut std::ffi::c_void,
  data: *mut std::ffi::c_void,
) {
  if data.is_null() {
    return;
  }

  let payload = unsafe { Box::<NativeCurrentThreadTaskHostPayload>::from_raw(data.cast()) };
  let delivery = payload.delivery;
  let callback_lease = if env.is_null() {
    None
  } else {
    contain_current_thread_task_host_unwind(|| {
      let lease = drive_current_thread_tasks(delivery.capability());
      #[cfg(test)]
      if lease.is_some() {
        run_native_task_host_after_drive_test_hook();
      }
      lease
    })
    .flatten()
  };
  let claimed = callback_lease.is_some();
  let completed = contain_current_thread_task_host_unwind(|| {
    if claimed {
      acknowledge_current_thread_task_delivery(delivery);
    } else {
      fail_current_thread_task_delivery(delivery);
    }
  });
  if completed.is_none() {
    let _ = contain_current_thread_task_host_unwind(|| {
      fail_current_thread_task_delivery(delivery);
    });
  }
  let _ = contain_current_thread_task_host_unwind(|| drop(payload));
  #[cfg(test)]
  run_native_task_host_after_payload_drop_test_hook();
  let _ = contain_current_thread_task_host_unwind(|| drop(callback_lease));
}

struct NativeCurrentThreadTaskHostPayload {
  delivery: CurrentThreadTaskDelivery,
  #[cfg(test)]
  drop_observer: Option<std::sync::Arc<std::sync::atomic::AtomicUsize>>,
}

impl NativeCurrentThreadTaskHostPayload {
  fn new(delivery: CurrentThreadTaskDelivery) -> Self {
    Self {
      delivery,
      #[cfg(test)]
      drop_observer: None,
    }
  }

  #[cfg(test)]
  fn with_drop_observer(
    delivery: CurrentThreadTaskDelivery,
    drop_observer: std::sync::Arc<std::sync::atomic::AtomicUsize>,
  ) -> Self {
    Self { delivery, drop_observer: Some(drop_observer) }
  }
}

#[cfg(test)]
impl Drop for NativeCurrentThreadTaskHostPayload {
  fn drop(&mut self) {
    if let Some(observer) = &self.drop_observer {
      observer.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
  }
}

// Bridges CurrentThread-flavor scheduling onto the owning JS thread through a
// threadsafe function; the `dead` / `environment_closing` flags turn late
// wakeups into no-ops instead of calls into a torn-down environment.
struct NativeCurrentThreadTaskHost {
  inner: std::sync::Arc<NativeCurrentThreadTaskHostInner>,
}

struct NativeCurrentThreadTaskHostInner {
  threadsafe_function: std::sync::Mutex<Option<usize>>,
  dead: std::sync::atomic::AtomicBool,
  environment_closing: std::sync::atomic::AtomicBool,
  host_registration: u64,
  registration: std::sync::Mutex<Option<CurrentThreadTaskDriverId>>,
}

static NATIVE_CURRENT_THREAD_TASK_HOSTS: std::sync::LazyLock<
  std::sync::Mutex<rustc_hash::FxHashMap<u64, std::sync::Weak<NativeCurrentThreadTaskHostInner>>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(rustc_hash::FxHashMap::default()));

fn registered_current_thread_task_host(
  id: u64,
) -> Option<std::sync::Arc<NativeCurrentThreadTaskHostInner>> {
  let mut registrations =
    NATIVE_CURRENT_THREAD_TASK_HOSTS.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
  let inner = registrations.get(&id).and_then(std::sync::Weak::upgrade);
  if inner.is_none() {
    registrations.remove(&id);
  }
  inner
}

impl NativeCurrentThreadTaskHostInner {
  fn new(env: &napi::Env, host_registration: u64) -> napi::Result<std::sync::Arc<Self>> {
    const ASYNC_RESOURCE_NAME: &[u8] = b"rolldown_current_thread_task_host";
    let name_len = isize::try_from(ASYNC_RESOURCE_NAME.len())
      .expect("the CurrentThread task-host resource name length must fit");
    let mut async_resource_name = ptr::null_mut();
    current_thread_task_host_napi_result(
      unsafe {
        napi::sys::napi_create_string_utf8(
          env.raw(),
          ASYNC_RESOURCE_NAME.as_ptr().cast(),
          name_len,
          &raw mut async_resource_name,
        )
      },
      "Failed to create the CurrentThread task-host async resource name",
    )?;

    let inner = std::sync::Arc::new(Self {
      threadsafe_function: std::sync::Mutex::default(),
      dead: std::sync::atomic::AtomicBool::new(false),
      environment_closing: std::sync::atomic::AtomicBool::new(false),
      host_registration,
      registration: std::sync::Mutex::default(),
    });
    let finalize_data =
      std::sync::Weak::into_raw(std::sync::Arc::downgrade(&inner)).cast_mut().cast();
    let mut threadsafe_function = ptr::null_mut();
    let create_status = unsafe {
      napi::sys::napi_create_threadsafe_function(
        env.raw(),
        ptr::null_mut(),
        ptr::null_mut(),
        async_resource_name,
        1,
        1,
        finalize_data,
        Some(finalize_native_current_thread_task_host),
        ptr::null_mut(),
        Some(call_native_current_thread_task_host),
        &raw mut threadsafe_function,
      )
    };
    if create_status != napi::sys::Status::napi_ok {
      unsafe {
        drop(std::sync::Weak::<Self>::from_raw(finalize_data.cast()));
      }
      return Err(napi::Error::new(
        napi::Status::from(create_status),
        "Failed to create the native CurrentThread task host",
      ));
    }
    *inner.threadsafe_function.lock().unwrap_or_else(std::sync::PoisonError::into_inner) =
      Some(threadsafe_function as usize);

    if let Err(error) = current_thread_task_host_napi_result(
      unsafe { napi::sys::napi_unref_threadsafe_function(env.raw(), threadsafe_function) },
      "Failed to unref the native CurrentThread task host",
    ) {
      inner.release_threadsafe_function(napi::sys::ThreadsafeFunctionReleaseMode::abort);
      return Err(error);
    }
    Ok(inner)
  }

  fn is_live(&self) -> bool {
    !self.dead.load(std::sync::atomic::Ordering::SeqCst)
      && !self.environment_closing.load(std::sync::atomic::Ordering::SeqCst)
      && self
        .threadsafe_function
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .is_some()
  }

  fn call_threadsafe_function_with(
    &self,
    data: *mut std::ffi::c_void,
    call: impl FnOnce(
      napi::sys::napi_threadsafe_function,
      *mut std::ffi::c_void,
    ) -> napi::sys::napi_status,
  ) -> Option<napi::sys::napi_status> {
    let mut slot =
      self.threadsafe_function.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if self.dead.load(std::sync::atomic::Ordering::SeqCst)
      || self.environment_closing.load(std::sync::atomic::Ordering::SeqCst)
    {
      return None;
    }
    let threadsafe_function = (*slot)? as napi::sys::napi_threadsafe_function;
    let status = call(threadsafe_function, data);
    if status == napi::sys::Status::napi_closing {
      // Node-API decrements this caller's acquisition before returning
      // `napi_closing`; the pointer is no longer safe for any later API call.
      self.environment_closing.store(true, std::sync::atomic::Ordering::SeqCst);
      slot.take();
    }
    Some(status)
  }

  fn dispatch(&self, delivery: CurrentThreadTaskDelivery) -> bool {
    let data: *mut std::ffi::c_void =
      Box::into_raw(Box::new(NativeCurrentThreadTaskHostPayload::new(delivery))).cast();
    let Some(status) =
      self.call_threadsafe_function_with(data, |threadsafe_function, data| unsafe {
        napi::sys::napi_call_threadsafe_function(
          threadsafe_function,
          data,
          napi::sys::ThreadsafeFunctionCallMode::nonblocking,
        )
      })
    else {
      unsafe {
        drop(Box::<NativeCurrentThreadTaskHostPayload>::from_raw(data.cast()));
      }
      return false;
    };
    if status != napi::sys::Status::napi_ok {
      unsafe {
        drop(Box::<NativeCurrentThreadTaskHostPayload>::from_raw(data.cast()));
      }
      return false;
    }
    true
  }

  fn finalized(&self) {
    self.environment_closing.store(true, std::sync::atomic::Ordering::SeqCst);
    // Finalization means Node is already destroying the TSFN. Invalidate the
    // pointer without calling back into Node from its own finalizer.
    self.threadsafe_function.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take();
    self.evict_inner(true, false);
  }

  fn environment_cleanup(&self) {
    self.environment_closing.store(true, std::sync::atomic::Ordering::SeqCst);
    // Cleanup is registered after the TSFN's own cleanup hook, so it normally
    // runs first. Keep owner retirement independent from registry eviction:
    // even a contained eviction panic must not retain the initial acquisition.
    let _ = contain_current_thread_task_host_unwind(|| self.evict_inner(true, false));
    self.release_threadsafe_function(napi::sys::ThreadsafeFunctionReleaseMode::release);
  }

  fn evict_after_sweep(&self) {
    let abort = !self.environment_closing.load(std::sync::atomic::Ordering::SeqCst);
    self.evict_inner(false, abort);
  }

  fn rollback(&self) {
    self.evict_inner(true, true);
  }

  fn evict_inner(&self, request_redispatch: bool, abort: bool) {
    self.dead.store(true, std::sync::atomic::Ordering::SeqCst);
    NATIVE_CURRENT_THREAD_TASK_HOSTS
      .lock()
      .unwrap_or_else(std::sync::PoisonError::into_inner)
      .remove(&self.host_registration);
    let registration =
      self.registration.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take();
    if let Some(id) = registration {
      unregister_current_thread_task_driver(id);
      if request_redispatch {
        // The dead host may have accepted a weak-TSFN callback that its env
        // discarded before delivery. Republish the same internal capability to
        // remaining live hosts through their exact delivery identities.
        request_current_thread_task_drain();
      }
    }
    if abort {
      self.release_threadsafe_function(napi::sys::ThreadsafeFunctionReleaseMode::abort);
    }
  }

  fn release_threadsafe_function(&self, mode: napi::sys::napi_threadsafe_function_release_mode) {
    self.release_threadsafe_function_with(mode, |threadsafe_function, mode| unsafe {
      napi::sys::napi_release_threadsafe_function(threadsafe_function, mode)
    });
  }

  fn release_threadsafe_function_with(
    &self,
    mode: napi::sys::napi_threadsafe_function_release_mode,
    release: impl FnOnce(
      napi::sys::napi_threadsafe_function,
      napi::sys::napi_threadsafe_function_release_mode,
    ) -> napi::sys::napi_status,
  ) {
    let threadsafe_function =
      self.threadsafe_function.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take();
    let Some(threadsafe_function) = threadsafe_function else {
      return;
    };
    let status = release(threadsafe_function as napi::sys::napi_threadsafe_function, mode);
    if status == napi::sys::Status::napi_closing {
      self.environment_closing.store(true, std::sync::atomic::Ordering::SeqCst);
    }
  }
}

impl CurrentThreadTaskDriver for NativeCurrentThreadTaskHost {
  fn dispatch(&self, delivery: CurrentThreadTaskDelivery) -> bool {
    self.inner.dispatch(delivery)
  }

  fn is_live(&self) -> bool {
    self.inner.is_live()
  }

  fn on_swept(&self) {
    // The registry is already selecting/dispatching a fallback. Avoid
    // recursively starting another selection pass from its sweep callback.
    self.inner.evict_after_sweep();
  }
}

fn reject_current_thread_task_host_callback(dispatch: Option<Unknown<'_>>) -> napi::Result<()> {
  if dispatch.is_none() {
    Ok(())
  } else {
    Err(napi::Error::new(
      napi::Status::InvalidArg,
      "registerCurrentThreadTaskHost does not accept a JavaScript callback",
    ))
  }
}

const CURRENT_THREAD_TASK_HOST_CONTRACT_VERSION: u32 = 4;

#[napi]
/// Return the native CurrentThread task-host ABI expected by the JavaScript
/// package before it invokes either async-runtime host registration. Version 4
/// reserves and validates an exact registration capability before host
/// installation performs side effects.
pub fn get_current_thread_task_host_contract_version() -> u32 {
  CURRENT_THREAD_TASK_HOST_CONTRACT_VERSION
}

#[napi]
/// Return whether one exact CurrentThread task- or timer-host registration is
/// still live. The JavaScript package revalidates its process-global marker on
/// every module evaluation so native eviction cannot leave a stale installed
/// bit that permanently suppresses replacement registration.
pub fn is_current_thread_host_registration_active(
  registration_high: u32,
  registration_low: u32,
) -> bool {
  let id = BindingHostRegistration::id(registration_high, registration_low);
  registered_current_thread_task_host(id).is_some_and(|inner| inner.is_live())
    || registered_timer_host(id).is_some_and(|inner| inner.is_live())
}

fn install_cleanup_hook_or_rollback<T>(
  install: impl FnOnce() -> napi::Result<T>,
  rollback: impl FnOnce(),
) -> napi::Result<()> {
  match install() {
    Ok(_) => Ok(()),
    Err(error) => {
      rollback();
      Err(error)
    }
  }
}

fn install_host_driver_registration<T>(
  dead: &std::sync::atomic::AtomicBool,
  registration: &std::sync::Mutex<Option<T>>,
  install: impl FnOnce() -> T,
  publish: impl FnOnce(),
  rollback: impl FnOnce(T),
) -> bool {
  // `install` may synchronously wake arbitrary task wakers. Keep it outside
  // the exact-registration mutex so a reentrant liveness sweep can evict this
  // host instead of deadlocking on the half-published registration.
  let mut installed = Some(install());
  {
    let mut slot = registration.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if !dead.load(std::sync::atomic::Ordering::SeqCst) {
      *slot = installed.take();
      // Publication is serialized with eviction by `registration`. This
      // callback must only update binding-owned indexes.
      publish();
    }
  }
  if let Some(installed) = installed {
    rollback(installed);
    false
  } else {
    true
  }
}

#[napi(ts_args_type = "registrationHigh: number, registrationLow: number, dispatch?: never")]
/// Install a native-owned host turn used to poll CurrentThread runnables
/// without re-entering arbitrary future waker locks. Called once per importing
/// environment. JavaScript callbacks are rejected synchronously.
pub fn register_current_thread_task_host(
  env: &napi::Env,
  registration_high: u32,
  registration_low: u32,
  dispatch: Option<Unknown<'_>>,
) -> napi::Result<()> {
  reject_current_thread_task_host_callback(dispatch)?;
  let host_registration = claim_host_registration_id(registration_high, registration_low)?;
  let inner = NativeCurrentThreadTaskHostInner::new(env, host_registration)?;
  {
    let mut slot = inner.registration.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    *slot =
      Some(register_current_thread_task_driver(std::sync::Arc::new(NativeCurrentThreadTaskHost {
        inner: std::sync::Arc::clone(&inner),
      })));
  }
  NATIVE_CURRENT_THREAD_TASK_HOSTS
    .lock()
    .unwrap_or_else(std::sync::PoisonError::into_inner)
    .insert(host_registration, std::sync::Arc::downgrade(&inner));
  request_current_thread_task_drain();
  let hook_inner = std::sync::Arc::clone(&inner);
  install_cleanup_hook_or_rollback(
    || {
      env.add_env_cleanup_hook(hook_inner, |inner| {
        let _ = contain_current_thread_task_host_unwind(|| inner.environment_cleanup());
      })
    },
    || inner.rollback(),
  )?;
  Ok(())
}

#[napi]
/// Evict exactly one native host installed by `registerCurrentThreadTaskHost`.
/// Managed workerd disposal uses this before environment cleanup so a later
/// throwing cleanup hook cannot leave the process-global driver selected.
pub fn unregister_current_thread_task_host(registration_high: u32, registration_low: u32) {
  let id = BindingHostRegistration::id(registration_high, registration_low);
  release_host_registration_id(id);
  if let Some(inner) = registered_current_thread_task_host(id) {
    inner.rollback();
  }
}

/// Host timer driver for the shared runtime's CurrentThread flavor:
/// `sleep_until` on the single-thread executor cannot park a
/// helper thread (none exists on threadless wasm), so it delegates each timer
/// to the host event loop through the JS callback registered at import --
/// `(id, ms) => new Promise((resolve) => setTimeout(resolve, ms))`, paired
/// with a cancellation callback that clears the timeout and resolves the
/// relay promise.
///
/// Per timer id: the FIRST `register` arms one host timeout via a detached
/// relay task; re-registers (`Sleep` re-polls) only refresh the stored waker.
/// `cancel` removes the pending waker and either invokes the host cancellation
/// callback or leaves a pre-arm tombstone for the accepted TSFN delivery. The
/// JS side clears the timeout and resolves its promise, so a dropped sleep
/// leaves neither a live timeout nor a detached relay task.
///
/// LIFETIME: each importing napi env registers its own
/// host, and a host dies WITH its env -- the weak threadsafe function does
/// not keep a worker's event loop alive, so a worker that imported the
/// binding can exit at any time and orphan its host. A dead host must never
/// keep timer duty (the registry would busy-fail every debounce against it),
/// so it is EVICTED -- proactively by the env-cleanup hook installed at
/// registration, and reactively by the `is_live` probe (the threadsafe
/// function's `aborted` flag) and by relay-call failure. Eviction wakes every
/// sleep armed here so each re-polls onto the registry's next live registrant
/// (see `TimerDriverRegistry`).
struct JsTimerHost {
  inner: std::sync::Arc<JsTimerHostInner>,
}

#[derive(Default)]
struct RelayIdAllocator {
  next: std::sync::atomic::AtomicU64,
}

impl RelayIdAllocator {
  fn reserve(&self) -> Result<u32, RelayIdExhausted> {
    self
      .next
      .fetch_update(
        std::sync::atomic::Ordering::Relaxed,
        std::sync::atomic::Ordering::Relaxed,
        |next| u32::try_from(next).is_ok().then_some(next + 1),
      )
      .map(|id| u32::try_from(id).expect("the checked relay id must fit in u32"))
      .map_err(|_| RelayIdExhausted)
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RelayIdExhausted;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
enum RelayCancellationAccounting {
  HostHealth = 0,
  CleanupOnly = 1,
}

struct HostTimerRelayHealth {
  cancellation_accounting: std::sync::atomic::AtomicU8,
  failure_recorded: std::sync::atomic::AtomicBool,
}

impl Default for HostTimerRelayHealth {
  fn default() -> Self {
    Self {
      cancellation_accounting: std::sync::atomic::AtomicU8::new(
        RelayCancellationAccounting::HostHealth as u8,
      ),
      failure_recorded: std::sync::atomic::AtomicBool::new(false),
    }
  }
}

impl HostTimerRelayHealth {
  fn set_cancellation_accounting(&self, accounting: RelayCancellationAccounting) {
    self.cancellation_accounting.store(accounting as u8, std::sync::atomic::Ordering::Release);
  }

  fn cancellation_accounting(&self) -> RelayCancellationAccounting {
    match self.cancellation_accounting.load(std::sync::atomic::Ordering::Acquire) {
      value if value == RelayCancellationAccounting::CleanupOnly as u8 => {
        RelayCancellationAccounting::CleanupOnly
      }
      _ => RelayCancellationAccounting::HostHealth,
    }
  }

  fn claim_failure(&self) -> bool {
    self
      .failure_recorded
      .compare_exchange(
        false,
        true,
        std::sync::atomic::Ordering::AcqRel,
        std::sync::atomic::Ordering::Acquire,
      )
      .is_ok()
  }

  fn failure_recorded(&self) -> bool {
    self.failure_recorded.load(std::sync::atomic::Ordering::Acquire)
  }
}

/// Consecutive NON-LIFETIME relay failures tolerated on one live host before
/// eviction. A transient failure (a one-off JS rejection, a queueing hiccup)
/// must not poison a live driver -- on a
/// main-only process that would leave NO driver and every later CT sleep
/// would hit the loud no-driver panic. But a PERSISTENTLY failing live
/// callback can never fire a timer either, so after this many consecutive
/// failures the host is evicted anyway (announced in the log). Reset on any
/// successful relay. Small on purpose: each strike costs one wasted arm/wake
/// round-trip for the affected sleep.
const HOST_TIMER_MAX_TRANSIENT_FAILURES: u32 = 3;

/// Does this relay error mean the HOST IS GONE (evict immediately), as
/// opposed to a callback failure on a live host (strike-counted)?
///
/// ERROR CLASSIFICATION: no message strings. A rejected JS promise is
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
///   teardown shapes (queue drained at env teardown, env died before the JS
///   promise settled) all coincide with the env being torn down, which the
///   probe observes directly.
///
/// Race walk: env dies between the error and the probe read -> the probe
/// reads dead -> evict: correct. Env alive at the probe but dying a
/// microsecond later -> strike now (the affected sleep is re-woken); the
/// death is then caught by the env-cleanup hook, by the aborted-probe sweep
/// at the next selection, or by the next relay failure (which will probe
/// dead) -> bounded, correct. A LIVE host's failure -- whatever its message
/// says -- takes the strike path.
fn should_evict_for_relay_error(status: napi::Status, host_is_live: bool) -> bool {
  status == napi::Status::Closing || !host_is_live
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HostTimerFailureAction {
  Duplicate,
  EvictHost,
  EvictHostAfterStrikes(u32),
  Retry(u32),
}

fn record_host_timer_failure(
  transient_failures: &std::sync::atomic::AtomicU32,
  relay_health: &HostTimerRelayHealth,
  status: napi::Status,
  host_is_live: bool,
) -> HostTimerFailureAction {
  if !relay_health.claim_failure() {
    return HostTimerFailureAction::Duplicate;
  }
  if should_evict_for_relay_error(status, host_is_live) {
    return HostTimerFailureAction::EvictHost;
  }
  let strikes = transient_failures.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
  if strikes >= HOST_TIMER_MAX_TRANSIENT_FAILURES {
    HostTimerFailureAction::EvictHostAfterStrikes(strikes)
  } else {
    HostTimerFailureAction::Retry(strikes)
  }
}

fn reset_host_timer_failures_after_success(
  transient_failures: &std::sync::atomic::AtomicU32,
  relay_health: &HostTimerRelayHealth,
) {
  if !relay_health.failure_recorded() {
    transient_failures.store(0, std::sync::atomic::Ordering::SeqCst);
  }
}

struct JsTimerHostInner {
  callback: JsCallback<FnArgs<(u32, f64)>, Promise<()>>,
  cancel_callback: JsCallback<FnArgs<(u32,)>, ()>,
  pending: std::sync::Mutex<rustc_hash::FxHashMap<TimerId, PendingHostTimer>>,
  relay_ids: RelayIdAllocator,
  /// Latched by [`JsTimerHostInner::evict`]: this host's env is gone (or its
  /// callback failed) and it must never serve another timer.
  dead: std::sync::atomic::AtomicBool,
  /// Exact JavaScript-facing capability used to unregister this host before
  /// emnapi starts running fallible environment cleanup hooks.
  host_registration: u64,
  /// This host's registration in the global driver registry. Installation and
  /// eviction use this mutex as their publication boundary, but the core
  /// registration call itself runs outside it because that call may wake
  /// arbitrary task wakers.
  registration: std::sync::Mutex<Option<TimerDriverId>>,
  /// Consecutive non-lifetime relay failures (see
  /// [`HOST_TIMER_MAX_TRANSIENT_FAILURES`]); reset on success.
  transient_failures: std::sync::atomic::AtomicU32,
}

static JS_TIMER_HOSTS: std::sync::LazyLock<
  std::sync::Mutex<rustc_hash::FxHashMap<u64, std::sync::Weak<JsTimerHostInner>>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(rustc_hash::FxHashMap::default()));

fn registered_timer_host(id: u64) -> Option<std::sync::Arc<JsTimerHostInner>> {
  let mut registrations = JS_TIMER_HOSTS.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
  let inner = registrations.get(&id).and_then(std::sync::Weak::upgrade);
  if inner.is_none() {
    registrations.remove(&id);
  }
  inner
}

struct PendingHostTimer {
  cancellation: Option<futures::channel::oneshot::Sender<()>>,
  relay_health: std::sync::Arc<HostTimerRelayHealth>,
  relay_id: u32,
  waker: std::task::Waker,
  schedule_state: RelayScheduleState,
}

impl PendingHostTimer {
  fn signal_native_cancellation(&mut self) {
    if let Some(sender) = self.cancellation.take() {
      let _ = sender.send(());
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RelayScheduleState {
  AwaitingCallback,
  CallbackComplete,
}

trait PendingRelayState: Send + Sync + 'static {
  fn take_pending_relay(
    &self,
    id: TimerId,
    relay_id: u32,
    accounting: RelayCancellationAccounting,
  ) -> Option<PendingHostTimer>;
  /// Record that the JavaScript schedule callback has returned. `true` means
  /// Rust cleanup already removed this exact relay, so cancellation must be
  /// submitted now that it cannot overtake timer creation.
  fn mark_relay_callback_complete(&self, id: TimerId, relay_id: u32) -> bool;
  fn cancel_relay(
    self: &std::sync::Arc<Self>,
    relay_id: u32,
    relay_health: std::sync::Arc<HostTimerRelayHealth>,
  );
}

struct PendingRelayDropGuard<T: PendingRelayState> {
  state: std::sync::Arc<T>,
  id: TimerId,
  relay_id: u32,
  cleanup_on_drop: bool,
}

impl<T: PendingRelayState> PendingRelayDropGuard<T> {
  fn new(state: std::sync::Arc<T>, id: TimerId, relay_id: u32) -> Self {
    Self { state, id, relay_id, cleanup_on_drop: true }
  }

  fn disarm(&mut self) {
    self.cleanup_on_drop = false;
  }
}

impl<T: PendingRelayState> Drop for PendingRelayDropGuard<T> {
  fn drop(&mut self) {
    if !self.cleanup_on_drop {
      return;
    }
    let Some(pending) = self.state.take_pending_relay(
      self.id,
      self.relay_id,
      RelayCancellationAccounting::HostHealth,
    ) else {
      return;
    };
    // An awaiting callback owns its exact, never-reused relay id and will
    // submit cancellation after JavaScript returns. Waking may re-enter timer
    // registration, but a later timer can never receive this id.
    retire_pending_relay(&self.state, pending);
  }
}

enum PendingHostTimerRegistration {
  Refreshed(std::task::Waker),
  Armed {
    relay_id: u32,
    cancellation: futures::channel::oneshot::Receiver<()>,
    relay_health: std::sync::Arc<HostTimerRelayHealth>,
  },
  Exhausted(std::task::Waker),
}

fn register_pending_host_timer_locked(
  pending: &mut rustc_hash::FxHashMap<TimerId, PendingHostTimer>,
  relay_ids: &RelayIdAllocator,
  id: TimerId,
  waker: std::task::Waker,
) -> PendingHostTimerRegistration {
  if let Some(slot) = pending.get_mut(&id) {
    return PendingHostTimerRegistration::Refreshed(std::mem::replace(&mut slot.waker, waker));
  }
  let Ok(relay_id) = relay_ids.reserve() else {
    return PendingHostTimerRegistration::Exhausted(waker);
  };
  let (cancellation_sender, cancellation) = futures::channel::oneshot::channel();
  let relay_health = std::sync::Arc::new(HostTimerRelayHealth::default());
  pending.insert(
    id,
    PendingHostTimer {
      cancellation: Some(cancellation_sender),
      relay_health: std::sync::Arc::clone(&relay_health),
      relay_id,
      waker,
      schedule_state: RelayScheduleState::AwaitingCallback,
    },
  );
  PendingHostTimerRegistration::Armed { relay_id, cancellation, relay_health }
}

#[cfg(test)]
fn register_pending_host_timer(
  pending: &std::sync::Mutex<rustc_hash::FxHashMap<TimerId, PendingHostTimer>>,
  relay_ids: &RelayIdAllocator,
  id: TimerId,
  waker: std::task::Waker,
) -> PendingHostTimerRegistration {
  let mut pending = pending.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
  register_pending_host_timer_locked(&mut pending, relay_ids, id, waker)
}

fn register_pending_host_timer_if_live(
  pending: &std::sync::Mutex<rustc_hash::FxHashMap<TimerId, PendingHostTimer>>,
  relay_ids: &RelayIdAllocator,
  id: TimerId,
  waker: std::task::Waker,
  is_live: impl FnOnce() -> bool,
) -> Result<PendingHostTimerRegistration, std::task::Waker> {
  let mut pending = pending.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
  if !is_live() {
    return Err(waker);
  }
  Ok(register_pending_host_timer_locked(&mut pending, relay_ids, id, waker))
}

fn take_pending_host_timers(
  pending: &std::sync::Mutex<rustc_hash::FxHashMap<TimerId, PendingHostTimer>>,
  accounting: RelayCancellationAccounting,
) -> rustc_hash::FxHashMap<TimerId, PendingHostTimer> {
  let mut pending = pending.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
  for timer in pending.values() {
    timer.relay_health.set_cancellation_accounting(accounting);
  }
  std::mem::take(&mut *pending)
}

fn take_pending_host_timer(
  pending: &std::sync::Mutex<rustc_hash::FxHashMap<TimerId, PendingHostTimer>>,
  id: TimerId,
  relay_id: u32,
  accounting: RelayCancellationAccounting,
) -> Option<PendingHostTimer> {
  let mut pending = pending.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
  if pending.get(&id).is_some_and(|slot| slot.relay_id == relay_id) {
    pending
      .get(&id)
      .expect("the matching pending relay must remain present")
      .relay_health
      .set_cancellation_accounting(accounting);
    pending.remove(&id)
  } else {
    None
  }
}

fn run_host_timer_cleanup_safely(cleanup: impl FnOnce()) {
  if let Err(payload) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(cleanup))
    && let Err(nested_payload) =
      std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(payload)))
  {
    // The original unwind is contained. A hostile payload destructor may
    // panic again; quarantine only that nested payload so its destructor
    // cannot unwind through the napi env cleanup hook either.
    std::mem::forget(nested_payload);
  }
}

fn retire_pending_relay<T: PendingRelayState>(
  state: &std::sync::Arc<T>,
  mut pending: PendingHostTimer,
) {
  // The detached relay must stop depending on JavaScript before cancellation
  // crosses the fallible TSFN boundary. A queue rejection or callback throw can
  // then affect diagnostics only, never relay retirement or runtime shutdown.
  pending.signal_native_cancellation();
  if pending.schedule_state == RelayScheduleState::CallbackComplete {
    let relay_health = std::sync::Arc::clone(&pending.relay_health);
    run_host_timer_cleanup_safely(|| state.cancel_relay(pending.relay_id, relay_health));
  }
  wake_host_timer_safely(pending.waker);
}

fn retire_pending_relays<T: PendingRelayState>(
  state: &std::sync::Arc<T>,
  pending: rustc_hash::FxHashMap<TimerId, PendingHostTimer>,
) {
  for (_, pending) in pending {
    retire_pending_relay(state, pending);
  }
}

fn complete_relay_schedule_callback<T: PendingRelayState, R>(
  state: &std::sync::Arc<T>,
  id: TimerId,
  relay_id: u32,
  relay_health: &std::sync::Arc<HostTimerRelayHealth>,
  result: R,
  deliver: impl FnOnce(R),
) {
  // This runs from napi-rs's return-value callback on the JavaScript call
  // stack, after the schedule function has returned but before delivery wakes
  // the Rust relay future. A terminal runnable drop therefore sees
  // CallbackComplete, while cleanup that won earlier is completed here.
  run_host_timer_cleanup_safely(|| {
    if state.mark_relay_callback_complete(id, relay_id) {
      state.cancel_relay(relay_id, std::sync::Arc::clone(relay_health));
    }
  });
  run_host_timer_cleanup_safely(|| deliver(result));
}

fn normalize_timer_schedule_result(
  result: napi::Result<napi::Either<Promise<()>, InvalidReturnValue>>,
) -> napi::Result<Promise<()>> {
  match result {
    Ok(napi::Either::A(promise)) => Ok(promise),
    Ok(napi::Either::B(invalid)) => Err(napi::Error::new(
      napi::Status::InvalidArg,
      format!(
        "The function returned `{}`, but expected `object`.",
        invalid.value_type.to_string().to_ascii_lowercase()
      ),
    )),
    Err(error) => Err(error),
  }
}

fn wake_host_timer_safely(waker: std::task::Waker) {
  // Host eviction runs from a napi env cleanup hook. A custom RawWaker must
  // not unwind through that FFI boundary or abort the remaining timer drain.
  // Borrowing for the wake keeps a panicking wake and a panicking destructor
  // in separate containment boundaries.
  run_host_timer_cleanup_safely(|| waker.wake_by_ref());
  run_host_timer_cleanup_safely(|| drop(waker));
}

fn drop_host_timer_waker_safely(waker: std::task::Waker) {
  run_host_timer_cleanup_safely(|| drop(waker));
}

fn recover_host_timer_failure(recover: impl FnOnce(), diagnostic: std::fmt::Arguments<'_>) {
  recover();
  run_host_timer_cleanup_safely(|| {
    use std::io::Write as _;
    let _ = writeln!(std::io::stderr().lock(), "{diagnostic}");
  });
}

impl JsTimerHostInner {
  fn lock_pending(
    &self,
  ) -> std::sync::MutexGuard<'_, rustc_hash::FxHashMap<TimerId, PendingHostTimer>> {
    self.pending.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
  }

  fn register_pending(
    &self,
    id: TimerId,
    waker: std::task::Waker,
  ) -> Result<PendingHostTimerRegistration, std::task::Waker> {
    // Eviction publishes `dead` before taking this same pending-map mutex to
    // drain. Registration therefore either enters the map before that drain,
    // or observes the dead/aborted host afterward and returns its waker intact.
    register_pending_host_timer_if_live(&self.pending, &self.relay_ids, id, waker, || {
      self.is_live()
    })
  }

  /// Can this host still deliver wakes? The `aborted` probe reads the
  /// threadsafe function's own closing flag, so a dying env is detected even
  /// before any eviction path ran.
  fn is_live(&self) -> bool {
    !self.dead.load(std::sync::atomic::Ordering::SeqCst)
      && !self.callback.aborted()
      && !self.cancel_callback.aborted()
  }

  fn handle_cancellation_failure(
    self: &std::sync::Arc<Self>,
    relay_id: u32,
    relay_health: &HostTimerRelayHealth,
    error: napi::Error,
    queue_failure: bool,
  ) {
    let operation = if queue_failure { "could not be queued" } else { "failed" };
    if relay_health.cancellation_accounting() == RelayCancellationAccounting::CleanupOnly {
      recover_host_timer_failure(
        || {},
        format_args!(
          "rolldown: host timer cancellation callback {operation} for relay {relay_id} during \
           cleanup: {error}"
        ),
      );
      return;
    }

    match record_host_timer_failure(
      &self.transient_failures,
      relay_health,
      error.status,
      self.is_live(),
    ) {
      HostTimerFailureAction::Duplicate => {
        recover_host_timer_failure(
          || {},
          format_args!(
            "rolldown: host timer cancellation callback {operation} for relay {relay_id} after \
             this relay failure was already accounted: {error}"
          ),
        );
      }
      HostTimerFailureAction::EvictHost => {
        recover_host_timer_failure(
          || self.evict(),
          format_args!(
            "rolldown: host timer cancellation callback {operation} for relay {relay_id} (host \
             gone, evicting): {error}"
          ),
        );
      }
      HostTimerFailureAction::EvictHostAfterStrikes(strikes) => {
        recover_host_timer_failure(
          || self.evict(),
          format_args!(
            "rolldown: host timer cancellation callback {operation} {strikes} times in a row, \
             evicting this timer host (relay {relay_id}): {error}"
          ),
        );
      }
      HostTimerFailureAction::Retry(strikes) => {
        recover_host_timer_failure(
          || {},
          format_args!(
            "rolldown: host timer cancellation callback {operation} for relay {relay_id} \
             ({strikes}/{HOST_TIMER_MAX_TRANSIENT_FAILURES} before eviction): {error}"
          ),
        );
      }
    }
  }

  fn cancel_relay(
    self: &std::sync::Arc<Self>,
    relay_id: u32,
    relay_health: std::sync::Arc<HostTimerRelayHealth>,
  ) {
    let callback_inner = std::sync::Arc::clone(self);
    let callback_health = std::sync::Arc::clone(&relay_health);
    let status = self.cancel_callback.call_with_return_value(
      FnArgs { data: (relay_id,) },
      ThreadsafeFunctionCallMode::NonBlocking,
      move |result, _| {
        let error = match result {
          Ok(napi::Either::A(())) => {
            if callback_health.cancellation_accounting() == RelayCancellationAccounting::HostHealth
            {
              reset_host_timer_failures_after_success(
                &callback_inner.transient_failures,
                &callback_health,
              );
            }
            return Ok(());
          }
          Ok(napi::Either::B(invalid)) => napi::Error::new(
            napi::Status::InvalidArg,
            format!(
              "The timer cancellation callback returned `{}`, but expected `undefined`.",
              invalid.value_type.to_string().to_ascii_lowercase()
            ),
          ),
          Err(error) => error,
        };
        callback_inner.handle_cancellation_failure(relay_id, &callback_health, error, false);
        Ok(())
      },
    );
    if status != napi::Status::Ok {
      let error = napi::Error::new(status, "Threadsafe timer cancellation callback call failed");
      self.handle_cancellation_failure(relay_id, &relay_health, error, true);
    }
  }

  fn mark_relay_callback_complete(&self, id: TimerId, relay_id: u32) -> bool {
    let mut pending = self.lock_pending();
    match pending.get_mut(&id) {
      Some(slot) if slot.relay_id == relay_id => {
        slot.schedule_state = RelayScheduleState::CallbackComplete;
        false
      }
      _ => true,
    }
  }

  fn take_pending_relay(
    &self,
    id: TimerId,
    relay_id: u32,
    accounting: RelayCancellationAccounting,
  ) -> Option<PendingHostTimer> {
    take_pending_host_timer(&self.pending, id, relay_id, accounting)
  }

  async fn invoke_schedule_callback(
    self: &std::sync::Arc<Self>,
    id: TimerId,
    relay_id: u32,
    ms: f64,
    relay_health: std::sync::Arc<HostTimerRelayHealth>,
  ) -> napi::Result<Promise<()>> {
    let (sender, receiver) = futures::channel::oneshot::channel();
    let callback_inner = std::sync::Arc::clone(self);
    let status = self.callback.call_with_return_value(
      FnArgs { data: (relay_id, ms) },
      ThreadsafeFunctionCallMode::NonBlocking,
      move |result, _| {
        complete_relay_schedule_callback(
          &callback_inner,
          id,
          relay_id,
          &relay_health,
          result,
          move |result| {
            let _ = sender.send(normalize_timer_schedule_result(result));
          },
        );
        Ok(())
      },
    );
    if status != napi::Status::Ok {
      return Err(napi::Error::new(status, "Threadsafe function call_async_catch failed"));
    }
    receiver.await.map_err(|_| {
      napi::Error::new(
        napi::Status::GenericFailure,
        "Receive value from threadsafe function sender failed",
      )
    })?
  }

  /// Remove this host from timer duty: latch `dead`, drop the registry
  /// entry, and wake every sleep armed here so each re-polls onto the next
  /// live registrant (absolute deadlines preserve the remaining time; with no
  /// live registrant left the re-poll fails LOUD in `rolldown_utils`).
  /// Idempotent -- the cleanup hook, the `is_live` race path, and the
  /// relay-failure backstop may all reach it.
  fn evict(self: &std::sync::Arc<Self>) {
    self.dead.store(true, std::sync::atomic::Ordering::SeqCst);
    let registration =
      self.registration.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take();
    JS_TIMER_HOSTS
      .lock()
      .unwrap_or_else(std::sync::PoisonError::into_inner)
      .remove(&self.host_registration);
    if let Some(id) = registration {
      unregister_timer_driver(id);
    }
    let pending = take_pending_host_timers(&self.pending, RelayCancellationAccounting::CleanupOnly);
    retire_pending_relays(self, pending);
  }
}

impl PendingRelayState for JsTimerHostInner {
  fn take_pending_relay(
    &self,
    id: TimerId,
    relay_id: u32,
    accounting: RelayCancellationAccounting,
  ) -> Option<PendingHostTimer> {
    JsTimerHostInner::take_pending_relay(self, id, relay_id, accounting)
  }

  fn mark_relay_callback_complete(&self, id: TimerId, relay_id: u32) -> bool {
    JsTimerHostInner::mark_relay_callback_complete(self, id, relay_id)
  }

  fn cancel_relay(
    self: &std::sync::Arc<Self>,
    relay_id: u32,
    relay_health: std::sync::Arc<HostTimerRelayHealth>,
  ) {
    JsTimerHostInner::cancel_relay(self, relay_id, relay_health);
  }
}

impl TimerDriver for JsTimerHost {
  fn register(&self, id: TimerId, deadline: std::time::Instant, waker: std::task::Waker) {
    if !self.inner.is_live() {
      // The selecting poll raced this host's death: make sure the eviction
      // bookkeeping ran, then wake immediately so the sleep re-selects from
      // the registry (which no longer offers this host).
      self.inner.evict();
      wake_host_timer_safely(waker);
      return;
    }
    let (relay_id, cancellation, relay_health) = match self.inner.register_pending(id, waker) {
      Err(waker) => {
        self.inner.evict();
        wake_host_timer_safely(waker);
        return;
      }
      Ok(PendingHostTimerRegistration::Refreshed(replaced_waker)) => {
        // A custom RawWaker destructor may re-enter timer code. `register_pending`
        // returns the old waker so its destructor runs after `pending` unlocks.
        drop_host_timer_waker_safely(replaced_waker);
        return;
      }
      Ok(PendingHostTimerRegistration::Armed { relay_id, cancellation, relay_health }) => {
        (relay_id, cancellation, relay_health)
      }
      Ok(PendingHostTimerRegistration::Exhausted(waker)) => {
        recover_host_timer_failure(
          || {
            self.inner.evict();
            wake_host_timer_safely(waker);
          },
          format_args!("rolldown: host timer relay id space exhausted, evicting this timer host"),
        );
        return;
      }
    };
    let ms = deadline.saturating_duration_since(std::time::Instant::now()).as_secs_f64() * 1000.0;
    let inner = std::sync::Arc::clone(&self.inner);
    // Build the guard outside the async body so destroying a never-polled
    // relay runnable still retires its pending timer.
    let relay_drop_guard = PendingRelayDropGuard::new(std::sync::Arc::clone(&inner), id, relay_id);
    let submission = try_spawn_detached(async move {
      let mut relay_drop_guard = relay_drop_guard;
      let schedule_started = std::sync::atomic::AtomicBool::new(false);
      let schedule = async {
        schedule_started.store(true, std::sync::atomic::Ordering::Release);
        match inner
          .invoke_schedule_callback(id, relay_id, ms, std::sync::Arc::clone(&relay_health))
          .await
        {
          Ok(promise) => promise.await,
          Err(error) => Err(error),
        }
      };
      futures::pin_mut!(cancellation);
      futures::pin_mut!(schedule);
      let result = match futures::future::select(cancellation, schedule).await {
        futures::future::Either::Left((_cancelled, schedule)) => {
          // Native cancellation won the race. The pending entry is already
          // gone (`cancel` removes it before signalling this oneshot), so the
          // drop guard has nothing left to clean up either way.
          relay_drop_guard.disarm();
          if !schedule_started.load(std::sync::atomic::Ordering::Acquire) {
            // The schedule callback was never invoked (the cancellation was
            // already pending at this task's first poll), so there is no host
            // result to observe -- and invoking the callback now would arm a
            // JS timer that nothing will ever cancel.
            return;
          }
          // The schedule callback is in flight. Conforming hosts settle the
          // schedule promise on cancellation (see `cancelTimer` in
          // workerd-timer-host.ts and the node host in timer-host.ts), and
          // that settlement shares this relay's health record with the
          // cancellation callback: whichever failure reaches Rust first
          // consumes the relay's one strike and the other is reported as
          // diagnostic-only (`Duplicate`, "already accounted"). Dropping the
          // schedule future here instead would leave the schedule-side
          // result unobserved and the shared accounting blind.
          schedule.await
        }
        futures::future::Either::Right((result, _cancellation)) => result,
      };
      match result {
        Ok(()) => {
          reset_host_timer_failures_after_success(&inner.transient_failures, &relay_health);
          if let Some(pending) =
            inner.take_pending_relay(id, relay_id, RelayCancellationAccounting::CleanupOnly)
          {
            wake_host_timer_safely(pending.waker);
          }
          relay_drop_guard.disarm();
        }
        Err(error) => {
          let action = record_host_timer_failure(
            &inner.transient_failures,
            &relay_health,
            error.status,
            inner.is_live(),
          );
          match action {
            HostTimerFailureAction::EvictHost => {
              recover_host_timer_failure(
                || inner.evict(),
                format_args!("rolldown: host timer callback failed (host gone, evicting): {error}"),
              );
            }
            HostTimerFailureAction::EvictHostAfterStrikes(strikes) => {
              recover_host_timer_failure(
                || inner.evict(),
                format_args!(
                  "rolldown: host timer callback failed {strikes} times in a row, evicting this \
                   timer host: {error}"
                ),
              );
            }
            HostTimerFailureAction::Retry(strikes) => {
              recover_host_timer_failure(
                || {
                  if let Some(pending) =
                    inner.take_pending_relay(id, relay_id, RelayCancellationAccounting::CleanupOnly)
                  {
                    // The callback may have synchronously armed a timeout before
                    // throwing, returning the wrong type, or producing a
                    // rejected Promise. Cleanup cancellation must not add a
                    // second strike for this same relay failure.
                    retire_pending_relay(&inner, pending);
                  }
                },
                format_args!(
                  "rolldown: host timer callback failed \
                   ({strikes}/{HOST_TIMER_MAX_TRANSIENT_FAILURES} before eviction): {error}"
                ),
              );
            }
            HostTimerFailureAction::Duplicate => {
              recover_host_timer_failure(
                || {
                  if let Some(pending) =
                    inner.take_pending_relay(id, relay_id, RelayCancellationAccounting::CleanupOnly)
                  {
                    retire_pending_relay(&inner, pending);
                  }
                },
                format_args!(
                  "rolldown: host timer callback failed after this relay failure was already \
                   accounted: {error}"
                ),
              );
            }
          }
          relay_drop_guard.disarm();
        }
      }
    });
    if let Err(rejected_relay) = submission {
      // The rejected future owns the exact-match guard. Dropping it removes
      // and wakes the pending timer before returning to the caller.
      drop(rejected_relay);
    }
  }

  fn cancel(&self, id: TimerId) {
    let pending = {
      let mut pending = self.inner.lock_pending();
      if let Some(timer) = pending.get(&id) {
        timer.relay_health.set_cancellation_accounting(RelayCancellationAccounting::HostHealth);
      }
      pending.remove(&id)
    };
    if let Some(mut pending) = pending {
      pending.signal_native_cancellation();
      if pending.schedule_state == RelayScheduleState::CallbackComplete {
        let relay_health = std::sync::Arc::clone(&pending.relay_health);
        self.inner.cancel_relay(pending.relay_id, relay_health);
      }
      drop_host_timer_waker_safely(pending.waker);
    }
  }

  fn is_live(&self) -> bool {
    self.inner.is_live()
  }

  fn on_swept(&self) {
    // The registry's selection sweep noticed this host's death (the
    // `aborted` probe can fire before the env-cleanup hook runs): run the
    // full eviction so every sleep pending here is woken into re-selection
    // instead of stranded. Idempotent with the hook and the relay backstop.
    self.inner.evict();
  }
}

#[napi(
  ts_args_type = "registrationHigh: number, registrationLow: number, schedule: (id: number, ms: number) => Promise<void>, cancel: (id: number) => void"
)]
/// Install the host timer callback backing the shared async runtime's
/// CurrentThread timers (watch-mode debounce). Called at import by every
/// binding-loading JS entry with paired setTimeout/clearTimeout callbacks; each
/// importing env (main thread and workers alike) registers its own host, and
/// every live host receives each timer.
pub fn register_timer_host(
  env: &napi::Env,
  registration_high: u32,
  registration_low: u32,
  schedule: JsCallback<FnArgs<(u32, f64)>, Promise<()>>,
  cancel: JsCallback<FnArgs<(u32,)>, ()>,
) -> napi::Result<()> {
  let host_registration = claim_host_registration_id(registration_high, registration_low)?;
  let inner = std::sync::Arc::new(JsTimerHostInner {
    callback: schedule,
    cancel_callback: cancel,
    pending: std::sync::Mutex::default(),
    relay_ids: RelayIdAllocator::default(),
    dead: std::sync::atomic::AtomicBool::new(false),
    host_registration,
    registration: std::sync::Mutex::default(),
    transient_failures: std::sync::atomic::AtomicU32::new(0),
  });
  if !install_host_driver_registration(
    &inner.dead,
    &inner.registration,
    || {
      register_timer_driver(std::sync::Arc::new(JsTimerHost {
        inner: std::sync::Arc::clone(&inner),
      }))
    },
    || {
      JS_TIMER_HOSTS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(host_registration, std::sync::Arc::downgrade(&inner));
    },
    unregister_timer_driver,
  ) {
    return Err(napi::Error::new(
      napi::Status::GenericFailure,
      "The CurrentThread timer host was evicted during registration",
    ));
  }
  // Proactive eviction at env teardown (worker exit): the primary lifetime
  // mechanism; the `aborted` probe and the relay-failure path in the driver
  // are the backstops for anything the hook cannot reach in time.
  let hook_inner = std::sync::Arc::clone(&inner);
  install_cleanup_hook_or_rollback(
    || {
      env.add_env_cleanup_hook(hook_inner, |inner| {
        run_host_timer_cleanup_safely(|| inner.evict());
      })
    },
    || run_host_timer_cleanup_safely(|| inner.evict()),
  )?;
  Ok(())
}

#[napi]
/// Evict exactly one callback installed by `registerTimerHost`.
/// Pending sleeps are woken so they can reselect another live environment.
pub fn unregister_timer_host(registration_high: u32, registration_low: u32) {
  let id = BindingHostRegistration::id(registration_high, registration_low);
  release_host_registration_id(id);
  if let Some(inner) = registered_timer_host(id) {
    inner.evict();
  }
}

#[napi_derive::module_init]
fn install_async_runtime_backend() {
  // Consume the SAME resolved snapshot the reporter and the capability export
  // read (the single config-resolution pipeline). `configure` validates the
  // already-normalized values, and the runtime controller's options remain
  // the reporting authority on this build.
  let resolved = resolved_runtime_config();
  let options = RuntimeOptions {
    flavor: resolved.flavor.into(),
    worker_threads: resolved.worker_threads,
    max_blocking_tasks: resolved.max_blocking_tasks,
    // Resolved from `ROLLDOWN_PARK_DEADLINE_MS` by the single resolver; the
    // runtime itself no longer reads the environment at executor construction.
    park_deadline: resolved.park_deadline_ms.map(std::time::Duration::from_millis),
    // The shared `napi-async-runtime` crate defaults to its own neutral
    // prefix; pin Rolldown's historical worker thread names explicitly.
    thread_name_prefix: "rolldown-runtime".to_string(),
  };
  configure(options).expect("Failed to configure the Rolldown async runtime");
  register_async_runtime(RolldownAsyncRuntime);
}

#[cfg(all(feature = "runtime-waker-teardown-test", not(target_family = "wasm")))]
struct RetainedSchedulerWakerProbe {
  sender: Option<std::sync::mpsc::Sender<std::task::Waker>>,
}

#[cfg(all(feature = "runtime-waker-teardown-test", not(target_family = "wasm")))]
impl Future for RetainedSchedulerWakerProbe {
  type Output = ();

  fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<()> {
    if let Some(sender) = self.sender.take() {
      let _ = sender.send(cx.waker().clone());
    }
    std::task::Poll::Pending
  }
}

#[cfg(all(feature = "runtime-waker-teardown-test", not(target_family = "wasm")))]
fn run_retained_scheduler_waker_probe(
  receiver: std::sync::mpsc::Receiver<std::task::Waker>,
  armed_path: std::path::PathBuf,
  release_path: std::path::PathBuf,
  completed_path: std::path::PathBuf,
) {
  let result = (|| -> Result<(), String> {
    let waker = receiver
      .recv()
      .map_err(|_| "scheduler task retired before publishing its waker".to_string())?;
    std::fs::write(&armed_path, b"armed")
      .map_err(|error| format!("failed to publish armed marker: {error}"))?;

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while !release_path.exists() {
      if std::time::Instant::now() >= deadline {
        return Err("timed out waiting for the post-teardown release marker".to_string());
      }
      std::thread::sleep(std::time::Duration::from_millis(5));
    }

    std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
      waker.wake_by_ref();
      drop(waker);
    }))
    .map_err(|_| "post-teardown scheduler waker invocation panicked".to_string())
  })();

  let status = match result {
    Ok(()) => "completed".to_string(),
    Err(error) => format!("error: {error}"),
  };
  let _ = std::fs::write(completed_path, status);
}

/// Test-only worker teardown probe. It retains a real shared-scheduler waker
/// on an external native thread until the caller publishes `release_path`.
#[cfg(all(feature = "runtime-waker-teardown-test", not(target_family = "wasm")))]
#[napi(js_name = "__rolldownTestRetainSchedulerWaker")]
pub fn retain_scheduler_waker_for_worker_teardown(
  armed_path: String,
  release_path: String,
  completed_path: String,
) -> napi::Result<()> {
  let (sender, receiver) = std::sync::mpsc::channel();
  std::thread::Builder::new()
    .name("rolldown-waker-teardown-test".to_string())
    .spawn(move || {
      run_retained_scheduler_waker_probe(
        receiver,
        armed_path.into(),
        release_path.into(),
        completed_path.into(),
      );
    })
    .map_err(|error| {
      napi::Error::from_reason(format!(
        "Failed to start the scheduler waker teardown probe thread: {error}"
      ))
    })?;

  try_spawn_detached(RetainedSchedulerWakerProbe { sender: Some(sender) }).map_err(|_| {
    napi::Error::from_reason("The shared async runtime rejected the scheduler waker teardown probe")
  })
}

/// Stop the real shared scheduler so the next N-API future submission is
/// rejected. Exported only by the dedicated async-runtime integration build.
#[cfg(all(feature = "runtime-submission-failure-test", not(target_family = "wasm")))]
#[napi(js_name = "__rolldownTestStopAsyncRuntime")]
pub fn stop_async_runtime_for_submission_failure_test() -> napi::Result<()> {
  shutdown().map_err(|error| napi::Error::from_reason(error.to_string()))
}

/// Restart the scheduler after `__rolldownTestStopAsyncRuntime`.
#[cfg(all(feature = "runtime-submission-failure-test", not(target_family = "wasm")))]
#[napi(js_name = "__rolldownTestStartAsyncRuntime")]
pub fn start_async_runtime_for_submission_failure_test() -> napi::Result<()> {
  start().map_err(|error| napi::Error::from_reason(error.to_string()))
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
  /// The scheduler the binding was compiled with: always 'shared' (the
  /// tokio-free shared runtime). The 'tokio' member of the union survives
  /// only for LEGACY bindings, whose JS support layer synthesizes
  /// `backend: 'tokio'` capability objects.
  #[napi(ts_type = "'tokio' | 'shared'")]
  pub backend: String,
  /// The executor flavor actually in effect (post-validation; this reflects
  /// a pre-first-use `configureAsyncRuntime` override).
  pub flavor: BindingRuntimeFlavor,
  /// The compile target: 'native', 'wasi' (threadless `wasm32-wasip1`) or
  /// 'wasi-threads' (`wasm32-wasip1-threads`).
  #[napi(ts_type = "'native' | 'wasi' | 'wasi-threads'")]
  pub target: String,
  /// Convenience: the binding is a WebAssembly/WASI artifact (`target !==
  /// 'native'`).
  pub wasi: bool,
  /// The binding runs the shared async runtime: always true. Survives for
  /// LEGACY bindings, whose JS support layer synthesizes `false`.
  pub async_runtime_build: bool,
  /// Work is scheduled across multiple threads (`flavor === 'MultiThread'`).
  pub threads: bool,
  /// A timer facility backs `sleep_until` (the watch-mode debounce). This is
  /// true on the MultiThread flavor (executor-owned timer heap). On the
  /// CurrentThread flavor timers are delegated to the host event loop, so
  /// this reads true while a LIVE `registerTimerHost` registrant exists.
  /// Every public package entry that loads the binding registers a host
  /// driver per importing env at import, so through any supported entry the
  /// answer is true; a registrant whose env died (an exited worker) is
  /// evicted and does NOT count. Only a raw binding loaded outside the
  /// supported entries can observe false.
  pub timers: bool,
  /// Binding dev mode is supported by THIS RUNTIME: true when native work can
  /// progress on a MultiThread executor, false on CurrentThread where
  /// `BindingDevEngine::run()` cannot complete its initial build.
  pub dev_supported: bool,
  /// Watch mode is supported by THIS ARTIFACT: static per artifact, true on
  /// both native flavors, false on every wasm artifact (watch on WASI stalls
  /// on the initial build). Deliberately independent of the live `timers`
  /// registration state -- it describes what the artifact can do, and every
  /// public entry registers the timer host the watch debounce needs before
  /// exposing any API.
  pub watch_supported: bool,
  /// An arbitrary `block_on` entered from the JavaScript host thread may await
  /// a JavaScript continuation without starving that continuation. Currently
  /// false on every artifact: MultiThread keeps native pool work progressing,
  /// but a foreign `block_on` still parks Node's main event-loop thread. This
  /// can become true only with a proven host-pumping/non-parking mechanism.
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

  // The runtime controller's validated options are the flavor authority:
  // they include a pre-first-use `configureAsyncRuntime` override, which the
  // load-time snapshot cannot know about.
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
  let threads = matches!(flavor, BindingRuntimeFlavor::MultiThread);
  BindingRuntimeCapabilities {
    backend: "shared".to_string(),
    flavor,
    target: target.to_string(),
    wasi,
    async_runtime_build: true,
    threads,
    timers,
    dev_supported: threads,
    // Static per artifact (see the field doc): the capability contract must
    // not depend on import order or registration state.
    watch_supported: !wasi,
    block_on_js_thread_safe: false,
  }
}

// Resolver tests are parameterized on the target, so every arm of the
// defaults table is exercised on any host. The runtime snapshot must win over
// later environment mutations: the environment is read exactly once inside
// the `OnceLock` initializer of `resolved_runtime_config`.
#[cfg(test)]
mod tests {
  use rolldown_utils::max_async_runtime_worker_threads;

  use super::{
    BindingHostRegistration, ResolvedRuntimeFlavor, ResolvedRuntimeTarget, RuntimeEnv,
    claim_host_registration_id, get_current_thread_task_host_contract_version,
    native_default_parallelism, parse_park_deadline_ms, reserve_current_thread_host_registration,
    resolve_runtime_config_for, unregister_current_thread_task_host,
  };
  use super::{
    BindingRuntimeFlavor, BindingRuntimeOptions, HOST_TIMER_MAX_TRANSIENT_FAILURES,
    HostTimerFailureAction, HostTimerRelayHealth, MAX_SAFE_JS_INTEGER,
    NATIVE_TASK_HOST_AFTER_DRIVE_TEST_HOOK, NATIVE_TASK_HOST_AFTER_PAYLOAD_DROP_TEST_HOOK,
    NativeCurrentThreadTaskHostInner, NativeCurrentThreadTaskHostPayload, PendingHostTimer,
    PendingHostTimerRegistration, PendingRelayDropGuard, PendingRelayState,
    RelayCancellationAccounting, RelayIdAllocator, RelayScheduleState, RolldownAsyncRuntime,
    call_native_current_thread_task_host, complete_relay_schedule_callback,
    install_cleanup_hook_or_rollback, install_host_driver_registration, record_host_timer_failure,
    recover_host_timer_failure, register_pending_host_timer, register_pending_host_timer_if_live,
    reset_host_timer_failures_after_success, retire_pending_relay, retire_pending_relays,
    safe_js_number, take_pending_host_timers, wake_host_timer_safely,
  };

  #[derive(Default)]
  struct TestPendingRelayState {
    pending: std::sync::Mutex<
      rustc_hash::FxHashMap<rolldown_utils::async_runtime::TimerId, PendingHostTimer>,
    >,
    relay_ids: RelayIdAllocator,
    cancelled_relays: std::sync::Mutex<Vec<u32>>,
    panic_on_cancel: std::sync::atomic::AtomicBool,
  }

  impl PendingRelayState for TestPendingRelayState {
    fn take_pending_relay(
      &self,
      id: rolldown_utils::async_runtime::TimerId,
      relay_id: u32,
      accounting: RelayCancellationAccounting,
    ) -> Option<PendingHostTimer> {
      let mut pending = self.pending.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
      if pending.get(&id).is_some_and(|slot| slot.relay_id == relay_id) {
        pending
          .get(&id)
          .expect("the matching test relay must remain present")
          .relay_health
          .set_cancellation_accounting(accounting);
        pending.remove(&id)
      } else {
        None
      }
    }

    fn mark_relay_callback_complete(
      &self,
      id: rolldown_utils::async_runtime::TimerId,
      relay_id: u32,
    ) -> bool {
      let mut pending = self.pending.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
      match pending.get_mut(&id) {
        Some(slot) if slot.relay_id == relay_id => {
          slot.schedule_state = RelayScheduleState::CallbackComplete;
          false
        }
        _ => true,
      }
    }

    fn cancel_relay(
      self: &std::sync::Arc<Self>,
      relay_id: u32,
      _relay_health: std::sync::Arc<HostTimerRelayHealth>,
    ) {
      self
        .cancelled_relays
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .push(relay_id);
      assert!(
        !self.panic_on_cancel.load(std::sync::atomic::Ordering::SeqCst),
        "intentional relay cancellation panic"
      );
    }
  }

  fn test_pending_relay_guard(
    state: &std::sync::Arc<TestPendingRelayState>,
    id: rolldown_utils::async_runtime::TimerId,
    waker: std::task::Waker,
    schedule_state: RelayScheduleState,
  ) -> (u32, std::sync::Arc<HostTimerRelayHealth>, PendingRelayDropGuard<TestPendingRelayState>) {
    let relay_id = state.relay_ids.reserve().expect("the test relay id must be available");
    let relay_health = std::sync::Arc::new(HostTimerRelayHealth::default());
    let replaced = state.pending.lock().unwrap_or_else(std::sync::PoisonError::into_inner).insert(
      id,
      PendingHostTimer {
        cancellation: None,
        relay_health: std::sync::Arc::clone(&relay_health),
        relay_id,
        waker,
        schedule_state,
      },
    );
    assert!(replaced.is_none(), "the test timer id must be vacant");
    (relay_id, relay_health, PendingRelayDropGuard::new(std::sync::Arc::clone(state), id, relay_id))
  }

  struct BudgetBoundaryTimerWait {
    state: std::sync::Arc<TestPendingRelayState>,
    timer_id: rolldown_utils::async_runtime::TimerId,
    schedule_state: RelayScheduleState,
    registered: bool,
  }

  impl std::future::Future for BudgetBoundaryTimerWait {
    type Output = ();

    fn poll(
      mut self: std::pin::Pin<&mut Self>,
      cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
      assert!(!self.registered, "the timer wait must be cancelled before a second poll");
      self.registered = true;
      let state = std::sync::Arc::clone(&self.state);
      let timer_id = self.timer_id;
      let (_, _, relay_drop_guard) =
        test_pending_relay_guard(&state, timer_id, cx.waker().clone(), self.schedule_state);
      rolldown_utils::async_runtime::spawn_detached(async move {
        let mut relay_drop_guard = relay_drop_guard;
        std::future::pending::<()>().await;
        relay_drop_guard.disarm();
      });
      std::task::Poll::Pending
    }
  }

  fn env() -> RuntimeEnv {
    RuntimeEnv::default()
  }

  #[test]
  fn current_thread_task_host_contract_version_is_stable() {
    assert_eq!(get_current_thread_task_host_contract_version(), 4);
  }

  #[test]
  fn host_registration_reservations_are_exact_and_single_use() {
    let claimed = reserve_current_thread_host_registration().unwrap();
    let claimed_id = BindingHostRegistration::id(claimed.high, claimed.low);
    assert_eq!(
      claim_host_registration_id(claimed.high, claimed.low).unwrap(),
      claimed_id,
      "the exact reserved capability must be claimable once"
    );
    assert!(
      claim_host_registration_id(claimed.high, claimed.low).is_err(),
      "a consumed registration capability must not be reusable"
    );

    let released = reserve_current_thread_host_registration().unwrap();
    unregister_current_thread_task_host(released.high, released.low);
    assert!(
      claim_host_registration_id(released.high, released.low).is_err(),
      "unregister must release a reservation before installation"
    );
  }

  fn native_task_host_with_raw_owner(raw: usize) -> NativeCurrentThreadTaskHostInner {
    NativeCurrentThreadTaskHostInner {
      threadsafe_function: std::sync::Mutex::new(Some(raw)),
      dead: std::sync::atomic::AtomicBool::new(false),
      environment_closing: std::sync::atomic::AtomicBool::new(false),
      host_registration: u64::MAX,
      registration: std::sync::Mutex::default(),
    }
  }

  #[test]
  fn native_task_host_competing_release_paths_retire_the_raw_owner_once() {
    let inner = std::sync::Arc::new(native_task_host_with_raw_owner(1));
    let releases = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
    let mut threads = Vec::new();

    for mode in [
      napi::sys::ThreadsafeFunctionReleaseMode::release,
      napi::sys::ThreadsafeFunctionReleaseMode::abort,
    ] {
      let inner = std::sync::Arc::clone(&inner);
      let releases = std::sync::Arc::clone(&releases);
      let barrier = std::sync::Arc::clone(&barrier);
      threads.push(std::thread::spawn(move || {
        barrier.wait();
        inner.release_threadsafe_function_with(mode, |threadsafe_function, _| {
          assert_eq!(threadsafe_function as usize, 1);
          releases.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
          napi::sys::Status::napi_ok
        });
      }));
    }

    barrier.wait();
    for thread in threads {
      thread.join().unwrap();
    }
    assert_eq!(releases.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert!(
      inner.threadsafe_function.lock().unwrap_or_else(std::sync::PoisonError::into_inner).is_none()
    );
  }

  #[test]
  fn native_task_host_closing_call_retires_owner_without_a_second_release() {
    let inner = native_task_host_with_raw_owner(1);
    let status = inner
      .call_threadsafe_function_with(std::ptr::null_mut(), |threadsafe_function, _| {
        assert_eq!(threadsafe_function as usize, 1);
        napi::sys::Status::napi_closing
      })
      .expect("the fake TSFN owner must be callable");

    assert_eq!(status, napi::sys::Status::napi_closing);
    assert!(inner.environment_closing.load(std::sync::atomic::Ordering::SeqCst));
    let releases = std::sync::atomic::AtomicUsize::new(0);
    inner.release_threadsafe_function_with(
      napi::sys::ThreadsafeFunctionReleaseMode::release,
      |_, _| {
        releases.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        napi::sys::Status::napi_ok
      },
    );
    assert_eq!(
      releases.load(std::sync::atomic::Ordering::SeqCst),
      0,
      "napi_closing already retired the initial acquisition and invalidated the pointer"
    );
  }

  #[test]
  fn host_registration_capability_round_trips_without_precision_loss() {
    for id in [1, u64::from(u32::MAX), u64::from(u32::MAX) + 1, u64::MAX] {
      let registration = BindingHostRegistration::from_id(id);
      assert_eq!(BindingHostRegistration::id(registration.high, registration.low), id);
    }
  }

  #[test]
  fn binding_runtime_options_convert_to_a_partial_core_patch() {
    use rolldown_utils::async_runtime::{RuntimeFlavor, RuntimeOptionsPatch};

    let patch = RuntimeOptionsPatch::try_from(BindingRuntimeOptions {
      flavor: Some(BindingRuntimeFlavor::CurrentThread),
      worker_threads: None,
      max_blocking_tasks: Some(7.0),
    })
    .expect("valid binding options must convert");

    assert!(matches!(patch.flavor, Some(RuntimeFlavor::CurrentThread)));
    assert_eq!(patch.worker_threads, None);
    assert_eq!(patch.max_blocking_tasks, Some(7));
  }

  #[test]
  fn binding_runtime_options_reject_unsafe_thread_counts() {
    use rolldown_utils::async_runtime::RuntimeOptionsPatch;

    for value in [-1.0, 0.0, 1.5, f64::NAN, f64::INFINITY, 257.0, f64::from(u32::MAX) + 1.0] {
      let error = RuntimeOptionsPatch::try_from(BindingRuntimeOptions {
        flavor: None,
        worker_threads: Some(value),
        max_blocking_tasks: None,
      })
      .expect_err("invalid worker thread counts must be rejected");
      assert!(
        error.reason.contains("`workerThreads` must be a positive integer"),
        "unexpected validation error: {}",
        error.reason
      );
    }

    for value in [-1.0, 257.0] {
      let error = RuntimeOptionsPatch::try_from(BindingRuntimeOptions {
        flavor: None,
        worker_threads: None,
        max_blocking_tasks: Some(value),
      })
      .expect_err("invalid blocking task counts must be rejected");
      assert!(
        error.reason.contains("`maxBlockingTasks` must be a positive integer"),
        "unexpected validation error: {}",
        error.reason
      );
    }
  }

  #[test]
  fn runtime_metrics_preserve_safe_javascript_integer_range() {
    const BEYOND_U32_JS_NUMBER: f64 = 4_294_967_296.0;
    const MAX_SAFE_JS_NUMBER: f64 = 9_007_199_254_740_991.0;

    let beyond_u32 = u64::from(u32::MAX) + 1;
    assert_eq!(safe_js_number(beyond_u32).to_bits(), BEYOND_U32_JS_NUMBER.to_bits());
    assert_eq!(safe_js_number(MAX_SAFE_JS_INTEGER).to_bits(), MAX_SAFE_JS_NUMBER.to_bits());
    assert_eq!(
      safe_js_number(MAX_SAFE_JS_INTEGER + 1).to_bits(),
      MAX_SAFE_JS_NUMBER.to_bits(),
      "metrics must remain exact JavaScript numbers instead of silently losing integer precision"
    );
  }

  #[test]
  fn relay_ids_fail_closed_without_reuse_at_u32_exhaustion() {
    let allocator = RelayIdAllocator::default();
    allocator.next.store(u64::from(u32::MAX), std::sync::atomic::Ordering::Relaxed);

    assert_eq!(allocator.reserve(), Ok(u32::MAX));
    assert!(
      allocator.reserve().is_err(),
      "the allocator must fail instead of wrapping to a reusable id"
    );
    assert!(
      allocator.reserve().is_err(),
      "exhaustion must remain stable instead of spinning or advancing toward wraparound"
    );

    let pending = std::sync::Mutex::new(rustc_hash::FxHashMap::default());
    let waker = futures::task::noop_waker();
    let exhausted_waker = match register_pending_host_timer(&pending, &allocator, 1, waker.clone())
    {
      PendingHostTimerRegistration::Exhausted(waker) => waker,
      PendingHostTimerRegistration::Refreshed(_) | PendingHostTimerRegistration::Armed { .. } => {
        panic!("an exhausted allocator must reject a new pending timer")
      }
    };
    assert!(
      exhausted_waker.will_wake(&waker),
      "allocator failure must return the attempted timer's waker intact"
    );
    assert!(
      pending.lock().unwrap_or_else(std::sync::PoisonError::into_inner).is_empty(),
      "an exhausted allocator must not publish a pending timer without a relay capability"
    );
  }

  #[test]
  fn host_timer_wake_and_drop_panics_are_contained_separately() {
    struct PanickingWakeAndDrop;

    impl std::task::Wake for PanickingWakeAndDrop {
      fn wake(self: std::sync::Arc<Self>) {
        panic!("intentional host timer waker panic");
      }

      fn wake_by_ref(self: &std::sync::Arc<Self>) {
        panic!("intentional host timer borrowed waker panic");
      }
    }

    impl Drop for PanickingWakeAndDrop {
      fn drop(&mut self) {
        panic!("intentional host timer waker destructor panic");
      }
    }

    let waker = std::task::Waker::from(std::sync::Arc::new(PanickingWakeAndDrop));
    wake_host_timer_safely(waker);
  }

  #[test]
  fn relay_drop_guard_cleans_only_its_matching_pending_relay() {
    struct CountingWake(std::sync::Arc<std::sync::atomic::AtomicUsize>);

    impl std::task::Wake for CountingWake {
      fn wake(self: std::sync::Arc<Self>) {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
      }
    }

    let state = std::sync::Arc::new(TestPendingRelayState::default());
    let wakes = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let (relay_id, _, guard) = test_pending_relay_guard(
      &state,
      1,
      std::task::Waker::from(std::sync::Arc::new(CountingWake(std::sync::Arc::clone(&wakes)))),
      RelayScheduleState::CallbackComplete,
    );
    drop(guard);

    assert!(state.pending.lock().unwrap_or_else(std::sync::PoisonError::into_inner).is_empty());
    assert_eq!(
      *state.cancelled_relays.lock().unwrap_or_else(std::sync::PoisonError::into_inner),
      [relay_id],
      "an armed relay must be cancelled before its waiter is woken"
    );
    assert_eq!(
      wakes.load(std::sync::atomic::Ordering::SeqCst),
      1,
      "terminal relay cleanup must wake the pending timer"
    );

    let stale_wakes = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let (stale_relay_id, _, stale_guard) = test_pending_relay_guard(
      &state,
      2,
      std::task::Waker::from(std::sync::Arc::new(CountingWake(std::sync::Arc::clone(
        &stale_wakes,
      )))),
      RelayScheduleState::AwaitingCallback,
    );
    let replacement_relay_id =
      state.relay_ids.reserve().expect("the replacement relay id must be available");
    assert_ne!(
      replacement_relay_id, stale_relay_id,
      "relay ids must never be reused within one host"
    );
    let replaced = state
      .pending
      .lock()
      .unwrap_or_else(std::sync::PoisonError::into_inner)
      .insert(
        2,
        PendingHostTimer {
          cancellation: None,
          relay_health: std::sync::Arc::new(HostTimerRelayHealth::default()),
          relay_id: replacement_relay_id,
          waker: futures::task::noop_waker(),
          schedule_state: RelayScheduleState::CallbackComplete,
        },
      )
      .expect("the original relay must still be pending");
    drop(replaced);
    drop(stale_guard);

    let replacement = state
      .pending
      .lock()
      .unwrap_or_else(std::sync::PoisonError::into_inner)
      .remove(&2)
      .expect("a stale guard must preserve the replacement relay");
    assert_eq!(replacement.relay_id, replacement_relay_id);
    assert_eq!(stale_wakes.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert_eq!(
      *state.cancelled_relays.lock().unwrap_or_else(std::sync::PoisonError::into_inner),
      [relay_id],
      "a stale guard must not cancel a replacement relay"
    );
    drop(replacement);

    let disarmed_wakes = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let (disarmed_relay_id, _, mut disarmed_guard) = test_pending_relay_guard(
      &state,
      3,
      std::task::Waker::from(std::sync::Arc::new(CountingWake(std::sync::Arc::clone(
        &disarmed_wakes,
      )))),
      RelayScheduleState::CallbackComplete,
    );
    disarmed_guard.disarm();
    drop(disarmed_guard);
    let disarmed_pending = state
      .pending
      .lock()
      .unwrap_or_else(std::sync::PoisonError::into_inner)
      .remove(&3)
      .expect("normal relay completion must retain ownership of pending cleanup");
    assert_eq!(disarmed_pending.relay_id, disarmed_relay_id);
    assert_eq!(disarmed_wakes.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert_eq!(
      *state.cancelled_relays.lock().unwrap_or_else(std::sync::PoisonError::into_inner),
      [relay_id],
      "a disarmed guard must not change normal relay cleanup semantics"
    );
    drop(disarmed_pending);
  }

  #[test]
  fn relay_callback_completion_precedes_delivery_and_closes_terminal_drop_window() {
    struct CountingWake(std::sync::Arc<std::sync::atomic::AtomicUsize>);

    impl std::task::Wake for CountingWake {
      fn wake(self: std::sync::Arc<Self>) {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
      }
    }

    let state = std::sync::Arc::new(TestPendingRelayState::default());
    let wakes = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let (relay_id, relay_health, guard) = test_pending_relay_guard(
      &state,
      1,
      std::task::Waker::from(std::sync::Arc::new(CountingWake(std::sync::Arc::clone(&wakes)))),
      RelayScheduleState::AwaitingCallback,
    );

    // Model napi-rs after the JavaScript schedule function returned and armed
    // its timeout, but before result delivery can re-poll the Rust relay.
    complete_relay_schedule_callback(&state, 1, relay_id, &relay_health, guard, drop);

    assert!(state.pending.lock().unwrap_or_else(std::sync::PoisonError::into_inner).is_empty());
    assert_eq!(
      *state.cancelled_relays.lock().unwrap_or_else(std::sync::PoisonError::into_inner),
      [relay_id],
      "terminal drop during return-value delivery must cancel the synchronously armed timeout"
    );
    assert_eq!(wakes.load(std::sync::atomic::Ordering::SeqCst), 1);
  }

  #[test]
  fn relay_callback_completion_finishes_cleanup_that_won_before_js_returned() {
    struct CountingWake(std::sync::Arc<std::sync::atomic::AtomicUsize>);

    impl std::task::Wake for CountingWake {
      fn wake(self: std::sync::Arc<Self>) {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
      }
    }

    let state = std::sync::Arc::new(TestPendingRelayState::default());
    let wakes = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let (relay_id, relay_health, guard) = test_pending_relay_guard(
      &state,
      1,
      std::task::Waker::from(std::sync::Arc::new(CountingWake(std::sync::Arc::clone(&wakes)))),
      RelayScheduleState::AwaitingCallback,
    );

    drop(guard);
    assert!(
      state.cancelled_relays.lock().unwrap_or_else(std::sync::PoisonError::into_inner).is_empty(),
      "cancellation must not overtake a schedule callback that has not returned"
    );

    complete_relay_schedule_callback(&state, 1, relay_id, &relay_health, (), drop);
    assert_eq!(
      *state.cancelled_relays.lock().unwrap_or_else(std::sync::PoisonError::into_inner),
      [relay_id],
      "the callback return handshake must finish exact cancellation after timer creation"
    );
    assert_eq!(wakes.load(std::sync::atomic::Ordering::SeqCst), 1);
  }

  #[test]
  fn relay_terminal_cleanup_contains_cancel_panics_and_still_wakes() {
    struct CountingWake(std::sync::Arc<std::sync::atomic::AtomicUsize>);

    impl std::task::Wake for CountingWake {
      fn wake(self: std::sync::Arc<Self>) {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
      }
    }

    let state = std::sync::Arc::new(TestPendingRelayState::default());
    state.panic_on_cancel.store(true, std::sync::atomic::Ordering::SeqCst);
    let wakes = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let (relay_id, _, guard) = test_pending_relay_guard(
      &state,
      1,
      std::task::Waker::from(std::sync::Arc::new(CountingWake(std::sync::Arc::clone(&wakes)))),
      RelayScheduleState::CallbackComplete,
    );

    drop(guard);

    assert_eq!(
      *state.cancelled_relays.lock().unwrap_or_else(std::sync::PoisonError::into_inner),
      [relay_id]
    );
    assert_eq!(
      wakes.load(std::sync::atomic::Ordering::SeqCst),
      1,
      "a panicking cancellation path must not suppress the pending timer wake"
    );
  }

  #[test]
  fn relay_native_cancellation_settles_before_fallible_host_cancellation() {
    use futures::FutureExt as _;

    let state = std::sync::Arc::new(TestPendingRelayState::default());
    state.panic_on_cancel.store(true, std::sync::atomic::Ordering::SeqCst);
    let registration =
      register_pending_host_timer(&state.pending, &state.relay_ids, 1, futures::task::noop_waker());
    let cancellation = match registration {
      PendingHostTimerRegistration::Armed { cancellation, .. } => cancellation,
      PendingHostTimerRegistration::Refreshed(_) | PendingHostTimerRegistration::Exhausted(_) => {
        panic!("the first timer registration must reserve a native cancellation relay")
      }
    };
    let mut pending = state
      .pending
      .lock()
      .unwrap_or_else(std::sync::PoisonError::into_inner)
      .remove(&1)
      .expect("the registered timer must remain pending");
    pending.schedule_state = RelayScheduleState::CallbackComplete;

    retire_pending_relay(&state, pending);

    assert!(
      matches!(cancellation.now_or_never(), Some(Ok(()))),
      "native relay retirement must publish before the contained host cancellation panic"
    );
  }

  #[test]
  fn relay_bulk_eviction_contains_each_cancel_panic_and_wakes_every_timer() {
    struct CountingWake(std::sync::Arc<std::sync::atomic::AtomicUsize>);

    impl std::task::Wake for CountingWake {
      fn wake(self: std::sync::Arc<Self>) {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
      }
    }

    let state = std::sync::Arc::new(TestPendingRelayState::default());
    state.panic_on_cancel.store(true, std::sync::atomic::Ordering::SeqCst);
    let wakes = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let pending = rustc_hash::FxHashMap::from_iter([7, 8].map(|relay_id| {
      (
        u64::from(relay_id),
        PendingHostTimer {
          cancellation: None,
          relay_health: std::sync::Arc::new(HostTimerRelayHealth::default()),
          relay_id,
          waker: std::task::Waker::from(std::sync::Arc::new(CountingWake(std::sync::Arc::clone(
            &wakes,
          )))),
          schedule_state: RelayScheduleState::CallbackComplete,
        },
      )
    }));

    retire_pending_relays(&state, pending);

    let mut cancelled =
      state.cancelled_relays.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone();
    cancelled.sort_unstable();
    assert_eq!(cancelled, [7, 8]);
    assert_eq!(
      wakes.load(std::sync::atomic::Ordering::SeqCst),
      2,
      "one panicking cancellation must not suppress any later pending timer wake"
    );
  }

  #[test]
  fn failed_relay_retirement_cancels_only_after_the_schedule_callback_returns() {
    struct CountingWake(std::sync::Arc<std::sync::atomic::AtomicUsize>);

    impl std::task::Wake for CountingWake {
      fn wake(self: std::sync::Arc<Self>) {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
      }
    }

    let state = std::sync::Arc::new(TestPendingRelayState::default());
    let wakes = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    retire_pending_relay(
      &state,
      PendingHostTimer {
        cancellation: None,
        relay_health: std::sync::Arc::new(HostTimerRelayHealth::default()),
        relay_id: 7,
        waker: std::task::Waker::from(std::sync::Arc::new(CountingWake(std::sync::Arc::clone(
          &wakes,
        )))),
        schedule_state: RelayScheduleState::CallbackComplete,
      },
    );
    retire_pending_relay(
      &state,
      PendingHostTimer {
        cancellation: None,
        relay_health: std::sync::Arc::new(HostTimerRelayHealth::default()),
        relay_id: 8,
        waker: std::task::Waker::from(std::sync::Arc::new(CountingWake(std::sync::Arc::clone(
          &wakes,
        )))),
        schedule_state: RelayScheduleState::AwaitingCallback,
      },
    );

    assert_eq!(
      *state.cancelled_relays.lock().unwrap_or_else(std::sync::PoisonError::into_inner),
      [7],
      "a callback-complete failure may have armed a timeout, while a failed TSFN submission did not"
    );
    assert_eq!(
      wakes.load(std::sync::atomic::Ordering::SeqCst),
      2,
      "both failed relays must wake their timer operations"
    );
  }

  #[test]
  fn host_timer_cleanup_contains_panicking_payload_drop_across_ffi() {
    const CHILD_ENV: &str = "ROLLDOWN_TEST_HOST_TIMER_PANIC_PAYLOAD_CHILD";

    if std::env::var_os(CHILD_ENV).is_some() {
      struct PanicOnDrop;

      impl Drop for PanicOnDrop {
        fn drop(&mut self) {
          panic!("intentional host timer panic payload destructor panic");
        }
      }

      struct PanickingWake;

      impl std::task::Wake for PanickingWake {
        fn wake(self: std::sync::Arc<Self>) {
          std::panic::panic_any(PanicOnDrop);
        }
      }

      extern "C" fn cleanup_shim() {
        let waker = std::task::Waker::from(std::sync::Arc::new(PanickingWake));
        wake_host_timer_safely(waker);
      }

      cleanup_shim();
      return;
    }

    let output = std::process::Command::new(std::env::current_exe().unwrap())
      .arg("--exact")
      .arg("async_runtime::tests::host_timer_cleanup_contains_panicking_payload_drop_across_ffi")
      .arg("--nocapture")
      .env(CHILD_ENV, "1")
      .output()
      .expect("the host timer cleanup subprocess must start");
    assert!(
      output.status.success(),
      "host timer cleanup must contain panic-payload destruction across the C ABI; status={:?}\nstdout={}\nstderr={}",
      output.status.code(),
      String::from_utf8_lossy(&output.stdout),
      String::from_utf8_lossy(&output.stderr)
    );
  }

  #[test]
  fn host_timer_failure_recovery_precedes_and_survives_diagnostic_panics() {
    struct PanickingDiagnostic;

    impl std::fmt::Display for PanickingDiagnostic {
      fn fmt(&self, _formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        panic!("intentional host timer diagnostic panic");
      }
    }

    let recovered = std::sync::atomic::AtomicBool::new(false);
    let diagnostic = PanickingDiagnostic;
    recover_host_timer_failure(
      || recovered.store(true, std::sync::atomic::Ordering::SeqCst),
      format_args!("{diagnostic}"),
    );
    assert!(
      recovered.load(std::sync::atomic::Ordering::SeqCst),
      "timer recovery must complete before fallible diagnostics run"
    );
  }

  #[test]
  fn host_timer_registry_drops_wakers_after_unlock() {
    struct LockCheckingDrop {
      pending: std::sync::Weak<
        std::sync::Mutex<
          rustc_hash::FxHashMap<rolldown_utils::async_runtime::TimerId, PendingHostTimer>,
        >,
      >,
      dropped: std::sync::Arc<std::sync::atomic::AtomicBool>,
    }

    #[expect(
      clippy::manual_noop_waker,
      reason = "the Arc-backed no-op waker exists to exercise its re-entrant destructor"
    )]
    impl std::task::Wake for LockCheckingDrop {
      fn wake(self: std::sync::Arc<Self>) {}
    }

    impl Drop for LockCheckingDrop {
      fn drop(&mut self) {
        let pending = self.pending.upgrade().expect("the pending registry must outlive its waker");
        let _guard = pending.try_lock().unwrap_or_else(|_| {
          panic!("a host-timer waker was dropped while the pending registry was locked")
        });
        self.dropped.store(true, std::sync::atomic::Ordering::SeqCst);
      }
    }

    let pending = std::sync::Arc::new(std::sync::Mutex::new(rustc_hash::FxHashMap::default()));
    let relay_ids = RelayIdAllocator::default();
    let dropped = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let old_waker = std::task::Waker::from(std::sync::Arc::new(LockCheckingDrop {
      pending: std::sync::Arc::downgrade(&pending),
      dropped: std::sync::Arc::clone(&dropped),
    }));
    assert!(matches!(
      register_pending_host_timer(&pending, &relay_ids, 1, old_waker),
      PendingHostTimerRegistration::Armed { .. }
    ));

    let replaced_waker =
      match register_pending_host_timer(&pending, &relay_ids, 1, futures::task::noop_waker()) {
        PendingHostTimerRegistration::Refreshed(waker) => waker,
        PendingHostTimerRegistration::Armed { .. } => {
          panic!("a re-poll must refresh the existing timer")
        }
        PendingHostTimerRegistration::Exhausted(_) => {
          panic!("the test allocator must not be exhausted")
        }
      };
    assert!(
      !dropped.load(std::sync::atomic::Ordering::SeqCst),
      "the helper must return ownership instead of dropping under its lock"
    );
    drop(replaced_waker);
    assert!(dropped.load(std::sync::atomic::Ordering::SeqCst));

    let evicted_waker_dropped = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let evicted_waker = std::task::Waker::from(std::sync::Arc::new(LockCheckingDrop {
      pending: std::sync::Arc::downgrade(&pending),
      dropped: std::sync::Arc::clone(&evicted_waker_dropped),
    }));
    assert!(matches!(
      register_pending_host_timer(&pending, &relay_ids, 2, evicted_waker),
      PendingHostTimerRegistration::Armed { .. }
    ));
    let evicted = take_pending_host_timers(&pending, RelayCancellationAccounting::CleanupOnly);
    assert!(
      !evicted_waker_dropped.load(std::sync::atomic::Ordering::SeqCst),
      "bulk eviction must move wakers out instead of dropping under its lock"
    );
    drop(evicted);
    assert!(evicted_waker_dropped.load(std::sync::atomic::Ordering::SeqCst));
  }

  #[test]
  fn host_timer_eviction_prevents_pending_registration_after_its_drain() {
    let pending = std::sync::Arc::new(std::sync::Mutex::new(rustc_hash::FxHashMap::default()));
    let relay_ids = std::sync::Arc::new(RelayIdAllocator::default());
    let dead = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut pending_guard = pending.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let (started_tx, started_rx) = std::sync::mpsc::sync_channel(1);
    let registration_pending = std::sync::Arc::clone(&pending);
    let registration_relay_ids = std::sync::Arc::clone(&relay_ids);
    let registration_dead = std::sync::Arc::clone(&dead);
    let registration = std::thread::spawn(move || {
      started_tx.send(()).unwrap();
      register_pending_host_timer_if_live(
        &registration_pending,
        &registration_relay_ids,
        1,
        futures::task::noop_waker(),
        || !registration_dead.load(std::sync::atomic::Ordering::SeqCst),
      )
    });

    started_rx.recv().unwrap();
    dead.store(true, std::sync::atomic::Ordering::SeqCst);
    let drained = std::mem::take(&mut *pending_guard);
    drop(pending_guard);
    drop(drained);

    assert!(
      registration.join().unwrap().is_err(),
      "registration arriving after death publication and the pending drain must be rejected"
    );
    assert!(
      pending.lock().unwrap_or_else(std::sync::PoisonError::into_inner).is_empty(),
      "a stale selected driver must not publish a new pending arm after eviction drained the map"
    );
  }

  #[test]
  fn cleanup_hook_registration_failure_rolls_back_host_registration() {
    let rollback_calls = std::sync::atomic::AtomicUsize::new(0);
    let result = install_cleanup_hook_or_rollback(
      || -> napi::Result<()> {
        Err(napi::Error::new(
          napi::Status::GenericFailure,
          "intentional cleanup-hook registration failure",
        ))
      },
      || {
        rollback_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
      },
    );

    assert!(result.is_err());
    assert_eq!(rollback_calls.load(std::sync::atomic::Ordering::SeqCst), 1);
  }

  #[test]
  fn host_driver_installation_allows_reentrant_eviction_before_publication() {
    let dead = std::sync::atomic::AtomicBool::new(false);
    let registration = std::sync::Mutex::new(None);
    let published = std::sync::atomic::AtomicBool::new(false);
    let rolled_back = std::sync::atomic::AtomicUsize::new(0);

    let installed = install_host_driver_registration(
      &dead,
      &registration,
      || {
        dead.store(true, std::sync::atomic::Ordering::SeqCst);
        let removed = registration
          .try_lock()
          .expect("the arbitrary installation callback must run outside the registration mutex")
          .take();
        assert!(removed.is_none(), "reentrant eviction must see no half-published registration");
        17_u64
      },
      || {
        published.store(true, std::sync::atomic::Ordering::SeqCst);
      },
      |id| {
        assert_eq!(id, 17);
        let guard =
          registration.try_lock().expect("rollback must run outside the registration mutex");
        drop(guard);
        rolled_back.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
      },
    );

    assert!(!installed, "reentrant eviction must reject the exported registration");
    assert!(!published.load(std::sync::atomic::Ordering::SeqCst));
    assert_eq!(rolled_back.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert!(registration.lock().unwrap().is_none());
  }

  fn resolve(target: ResolvedRuntimeTarget, env: &RuntimeEnv) -> super::ResolvedRuntimeConfig {
    resolve_runtime_config_for(target, env)
  }

  #[test]
  fn rolldown_runtime_rejects_after_shutdown_and_accepts_after_restart() {
    use std::{
      sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
      },
      time::Duration,
    };

    napi::bindgen_prelude::AsyncRuntime::start(&RolldownAsyncRuntime)
      .expect("the adapter runtime must start");
    rolldown_utils::async_runtime::reset_metrics();
    let (first_tx, first_rx) = mpsc::channel();
    napi::bindgen_prelude::AsyncRuntime::spawn_blocking(
      &RolldownAsyncRuntime,
      Box::new(move || {
        first_tx.send(()).expect("test receiver must still be listening");
      }),
    )
    .unwrap_or_else(|_| panic!("the shared blocking lane must accept napi work"));
    first_rx
      .recv_timeout(Duration::from_secs(5))
      .expect("the shared blocking lane must execute accepted napi work");

    napi::bindgen_prelude::AsyncRuntime::shutdown(&RolldownAsyncRuntime)
      .expect("the adapter runtime must shut down");
    let block_on_ran = Arc::new(AtomicBool::new(false));
    let block_on_ran_future = Arc::clone(&block_on_ran);
    let mut block_on_future = std::pin::pin!(async move {
      block_on_ran_future.store(true, Ordering::SeqCst);
    });
    let block_on_error = napi::bindgen_prelude::AsyncRuntime::block_on(
      &RolldownAsyncRuntime,
      block_on_future.as_mut(),
    )
    .expect_err("the adapter must reject block_on while stopped");
    assert_eq!(
      block_on_error.reason,
      "the async runtime is stopped; call start before submitting work"
    );
    assert!(!block_on_ran.load(Ordering::SeqCst), "rejected block_on must not poll its future");

    let rejected_ran = Arc::new(AtomicBool::new(false));
    let rejected_ran_work = Arc::clone(&rejected_ran);
    let (rejected_retry_tx, rejected_retry_rx) = mpsc::channel();
    let rejected = napi::bindgen_prelude::AsyncRuntime::spawn_blocking(
      &RolldownAsyncRuntime,
      Box::new(move || {
        rejected_ran_work.store(true, Ordering::SeqCst);
        rejected_retry_tx.send(()).expect("test receiver must still be listening");
      }),
    )
    .expect_err("the adapter must reject work while stopped");
    assert!(!rejected_ran.load(Ordering::SeqCst));
    let (rejected, error) = rejected.into_parts();
    assert_eq!(error.reason, "the async runtime is stopped; call start before submitting work");

    napi::bindgen_prelude::AsyncRuntime::start(&RolldownAsyncRuntime)
      .expect("the adapter runtime must restart");
    napi::bindgen_prelude::AsyncRuntime::block_on(&RolldownAsyncRuntime, block_on_future.as_mut())
      .expect("the restarted adapter runtime must accept the retained future");
    assert!(block_on_ran.load(Ordering::SeqCst));

    napi::bindgen_prelude::AsyncRuntime::spawn_blocking(&RolldownAsyncRuntime, rejected)
      .unwrap_or_else(|_| panic!("the restarted runtime must accept the retained napi work"));
    rejected_retry_rx
      .recv_timeout(Duration::from_secs(5))
      .expect("the restarted runtime must execute the retained napi work");
    assert!(rejected_ran.load(Ordering::SeqCst), "rejected work must be returned intact");
    assert!(
      rolldown_utils::async_runtime::metrics().blocking_tasks_started >= 2,
      "both accepted adapter submissions should reach the shared blocking scheduler"
    );
  }

  #[cfg(not(target_family = "wasm"))]
  #[test]
  fn native_task_host_null_env_retires_payload_and_recovers_exact_delivery() {
    const CHILD_ENV: &str = "ROLLDOWN_TEST_NATIVE_TASK_HOST_NULL_ENV_CHILD";

    if std::env::var_os(CHILD_ENV).is_some() {
      use std::sync::{Arc, mpsc};
      use std::time::Duration;

      use rolldown_utils::async_runtime::{
        CurrentThreadTaskDelivery, CurrentThreadTaskDriver, RuntimeFlavor, RuntimeOptions,
        configure, register_current_thread_task_driver, shutdown, spawn_detached, start,
        unregister_current_thread_task_driver,
      };

      struct RecordingTaskDriver {
        dispatches: mpsc::Sender<CurrentThreadTaskDelivery>,
      }

      impl CurrentThreadTaskDriver for RecordingTaskDriver {
        fn dispatch(&self, delivery: CurrentThreadTaskDelivery) -> bool {
          self.dispatches.send(delivery).is_ok()
        }
      }

      configure(RuntimeOptions {
        flavor: RuntimeFlavor::CurrentThread,
        worker_threads: 1,
        max_blocking_tasks: 1,
        ..RuntimeOptions::default()
      })
      .expect("the isolated runtime must accept CurrentThread configuration");
      start().expect("the isolated CurrentThread runtime must start");

      let (dispatch_tx, dispatch_rx) = mpsc::channel();
      let driver_id = register_current_thread_task_driver(Arc::new(RecordingTaskDriver {
        dispatches: dispatch_tx,
      }));
      let (completed_tx, completed_rx) = mpsc::channel();
      spawn_detached(async move {
        completed_tx.send(()).expect("the completion observer must remain live");
      });

      let payload_drops = Arc::new(std::sync::atomic::AtomicUsize::new(0));
      let first_delivery = dispatch_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("the queued task must publish its first host delivery");
      let first_payload =
        Box::into_raw(Box::new(NativeCurrentThreadTaskHostPayload::with_drop_observer(
          first_delivery,
          Arc::clone(&payload_drops),
        )))
        .cast();
      unsafe {
        call_native_current_thread_task_host(
          std::ptr::null_mut(),
          std::ptr::null_mut(),
          std::ptr::null_mut(),
          first_payload,
        );
      }
      assert_eq!(
        payload_drops.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "the null-env callback must destroy its queued payload exactly once"
      );

      let replacement_delivery = dispatch_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("the exact failed delivery must publish one replacement");
      assert_ne!(
        replacement_delivery, first_delivery,
        "recovery must use a fresh registration-scoped delivery capability"
      );
      let replacement_payload =
        Box::into_raw(Box::new(NativeCurrentThreadTaskHostPayload::with_drop_observer(
          replacement_delivery,
          Arc::clone(&payload_drops),
        )))
        .cast();
      let fake_env = std::ptr::NonNull::<std::ffi::c_void>::dangling().as_ptr().cast();
      unsafe {
        call_native_current_thread_task_host(
          fake_env,
          std::ptr::null_mut(),
          std::ptr::null_mut(),
          replacement_payload,
        );
      }

      completed_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("the replacement host delivery must run the queued task");
      assert_eq!(
        payload_drops.load(std::sync::atomic::Ordering::SeqCst),
        2,
        "successful recovery must also destroy its queued payload exactly once"
      );
      assert!(
        matches!(dispatch_rx.try_recv(), Err(mpsc::TryRecvError::Empty)),
        "successful replacement acknowledgement must not publish another delivery"
      );

      unregister_current_thread_task_driver(driver_id);
      shutdown().expect("the isolated CurrentThread runtime must shut down cleanly");
      return;
    }

    let output = std::process::Command::new(std::env::current_exe().unwrap())
      .arg("--exact")
      .arg(
        "async_runtime::tests::native_task_host_null_env_retires_payload_and_recovers_exact_delivery",
      )
      .arg("--nocapture")
      .env(CHILD_ENV, "1")
      .output()
      .expect("the null-env task-host subprocess must start");
    assert!(
      output.status.success(),
      "the null-env task-host regression failed; status={:?}\nstdout={}\nstderr={}",
      output.status.code(),
      String::from_utf8_lossy(&output.stdout),
      String::from_utf8_lossy(&output.stderr)
    );
  }

  #[cfg(not(target_family = "wasm"))]
  #[test]
  fn native_task_host_callback_lease_covers_ack_payload_and_restart() {
    const CHILD_ENV: &str = "ROLLDOWN_TEST_NATIVE_TASK_HOST_CALLBACK_LEASE_CHILD";

    if std::env::var_os(CHILD_ENV).is_some() {
      use std::sync::{Arc, mpsc};
      use std::time::{Duration, Instant};

      use rolldown_utils::async_runtime::{
        CurrentThreadTaskDelivery, CurrentThreadTaskDriver, RuntimeFlavor, RuntimeOptions,
        configure, register_current_thread_task_driver, shutdown, spawn_detached, start,
        try_spawn_detached, unregister_current_thread_task_driver,
      };

      struct RecordingTaskDriver {
        dispatches: mpsc::Sender<CurrentThreadTaskDelivery>,
      }

      impl CurrentThreadTaskDriver for RecordingTaskDriver {
        fn dispatch(&self, delivery: CurrentThreadTaskDelivery) -> bool {
          self.dispatches.send(delivery).is_ok()
        }
      }

      configure(RuntimeOptions {
        flavor: RuntimeFlavor::CurrentThread,
        worker_threads: 1,
        max_blocking_tasks: 1,
        ..RuntimeOptions::default()
      })
      .expect("the isolated runtime must accept CurrentThread configuration");
      start().expect("the isolated CurrentThread runtime must start");

      let (dispatch_tx, dispatch_rx) = mpsc::channel();
      let driver_id = register_current_thread_task_driver(Arc::new(RecordingTaskDriver {
        dispatches: dispatch_tx,
      }));
      let (task_completed_tx, task_completed_rx) = mpsc::channel();
      spawn_detached(async move {
        task_completed_tx.send(()).expect("the task completion observer must remain live");
      });
      let delivery = dispatch_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("the queued task must publish one native host delivery");

      let payload_drops = Arc::new(std::sync::atomic::AtomicUsize::new(0));
      let payload = Box::into_raw(Box::new(NativeCurrentThreadTaskHostPayload::with_drop_observer(
        delivery,
        Arc::clone(&payload_drops),
      )))
      .cast::<std::ffi::c_void>() as usize;
      let (after_drive_tx, after_drive_rx) = mpsc::channel();
      let (release_after_drive_tx, release_after_drive_rx) = mpsc::channel();
      let (after_payload_tx, after_payload_rx) = mpsc::channel();
      let (release_after_payload_tx, release_after_payload_rx) = mpsc::channel();
      let callback = std::thread::spawn(move || {
        NATIVE_TASK_HOST_AFTER_DRIVE_TEST_HOOK.with(|slot| {
          *slot.borrow_mut() = Some(Box::new(move || {
            after_drive_tx.send(()).unwrap();
            release_after_drive_rx.recv().unwrap();
          }));
        });
        NATIVE_TASK_HOST_AFTER_PAYLOAD_DROP_TEST_HOOK.with(|slot| {
          *slot.borrow_mut() = Some(Box::new(move || {
            after_payload_tx.send(()).unwrap();
            release_after_payload_rx.recv().unwrap();
          }));
        });
        let fake_env = std::ptr::NonNull::<std::ffi::c_void>::dangling().as_ptr().cast();
        unsafe {
          call_native_current_thread_task_host(
            fake_env,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            payload as *mut std::ffi::c_void,
          );
        }
      });

      after_drive_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("the native callback must pause after driving the host turn");
      task_completed_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("the host turn must run the queued task before acknowledgement");

      let (shutdown_tx, shutdown_rx) = mpsc::channel();
      let shutdown_thread = std::thread::spawn(move || {
        shutdown_tx.send(shutdown()).unwrap();
      });
      let stopping_deadline = Instant::now() + Duration::from_secs(2);
      loop {
        match try_spawn_detached(async {}) {
          Ok(()) => {
            assert!(
              Instant::now() < stopping_deadline,
              "shutdown did not publish the stopping lifecycle"
            );
            std::thread::yield_now();
          }
          Err(future) => {
            drop(future);
            break;
          }
        }
      }
      assert!(
        shutdown_rx.recv_timeout(Duration::from_millis(200)).is_err(),
        "shutdown must wait after drive until delivery acknowledgement and payload destruction"
      );

      let (restart_tx, restart_rx) = mpsc::channel();
      let restart_thread = std::thread::spawn(move || {
        start().expect("the runtime must restart after the old callback retires");
        restart_tx.send(()).unwrap();
      });
      assert!(
        restart_rx.recv_timeout(Duration::from_millis(200)).is_err(),
        "restart must not overlap the old callback before acknowledgement"
      );

      release_after_drive_tx.send(()).unwrap();
      after_payload_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("the callback must pause after acknowledging and destroying its payload");
      assert_eq!(
        payload_drops.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "the acknowledged callback must destroy its payload before releasing the lease"
      );
      assert!(
        shutdown_rx.recv_timeout(Duration::from_millis(200)).is_err(),
        "shutdown must remain blocked after acknowledgement and payload destruction"
      );
      assert!(
        restart_rx.recv_timeout(Duration::from_millis(200)).is_err(),
        "restart must remain blocked until the callback lease retires"
      );

      release_after_payload_tx.send(()).unwrap();
      callback.join().unwrap();
      shutdown_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("shutdown must finish after the callback lease retires")
        .unwrap();
      restart_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("restart must finish after old-generation shutdown");
      shutdown_thread.join().unwrap();
      restart_thread.join().unwrap();

      unregister_current_thread_task_driver(driver_id);
      shutdown().expect("the replacement CurrentThread runtime must shut down cleanly");
      return;
    }

    let output = std::process::Command::new(std::env::current_exe().unwrap())
      .arg("--exact")
      .arg("async_runtime::tests::native_task_host_callback_lease_covers_ack_payload_and_restart")
      .arg("--nocapture")
      .env(CHILD_ENV, "1")
      .output()
      .expect("the native task-host callback lease subprocess must start");
    assert!(
      output.status.success(),
      "the native task-host callback lease regression failed; status={:?}\nstdout={}\nstderr={}",
      output.status.code(),
      String::from_utf8_lossy(&output.stdout),
      String::from_utf8_lossy(&output.stderr)
    );
  }

  #[cfg(not(target_family = "wasm"))]
  #[test]
  fn timer_relay_terminal_drop_at_host_turn_budget_settles_without_later_work() {
    const CHILD_ENV: &str = "ROLLDOWN_TEST_TIMER_RELAY_TERMINAL_DROP_CHILD";
    const HOST_TURN_RUNNABLE_BUDGET: usize = 64;

    if let Some(relay_state) = std::env::var_os(CHILD_ENV) {
      use std::sync::{Arc, mpsc};
      use std::time::Duration;

      use futures::FutureExt as _;
      use rolldown_utils::async_runtime::{
        CurrentThreadTaskDelivery, CurrentThreadTaskDriver, RuntimeFlavor, RuntimeOptions,
        acknowledge_current_thread_task_delivery, cancel_current_thread_task_dispatch, configure,
        drive_current_thread_tasks, metrics, register_current_thread_task_driver, reset_metrics,
        shutdown, spawn, spawn_detached, start, unregister_current_thread_task_driver,
      };

      struct RecordingTaskDriver {
        dispatches: mpsc::Sender<CurrentThreadTaskDelivery>,
      }

      impl CurrentThreadTaskDriver for RecordingTaskDriver {
        fn dispatch(&self, delivery: CurrentThreadTaskDelivery) -> bool {
          self.dispatches.send(delivery).is_ok()
        }
      }

      let schedule_state = if relay_state == "complete" {
        RelayScheduleState::CallbackComplete
      } else {
        RelayScheduleState::AwaitingCallback
      };

      let options = RuntimeOptions {
        flavor: RuntimeFlavor::CurrentThread,
        worker_threads: 1,
        max_blocking_tasks: 1,
        ..RuntimeOptions::default()
      };
      configure(options).expect("the isolated runtime must accept CurrentThread configuration");
      start().expect("the isolated CurrentThread runtime must start");
      reset_metrics();

      let (dispatch_tx, dispatch_rx) = mpsc::channel();
      let driver_id = register_current_thread_task_driver(Arc::new(RecordingTaskDriver {
        dispatches: dispatch_tx,
      }));

      for _ in 0..HOST_TURN_RUNNABLE_BUDGET - 1 {
        spawn_detached(async {});
      }
      let state = Arc::new(TestPendingRelayState::default());
      let timer_task = spawn(BudgetBoundaryTimerWait {
        state: Arc::clone(&state),
        timer_id: 1,
        schedule_state,
        registered: false,
      });
      let first_dispatch = dispatch_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("the initial host turn must be published");
      assert!(
        matches!(dispatch_rx.try_recv(), Err(mpsc::TryRecvError::Empty)),
        "all 64 initial runnables must coalesce behind one host turn"
      );

      let callback_lease = drive_current_thread_tasks(first_dispatch.capability())
        .expect("the exact host delivery must be admitted");
      acknowledge_current_thread_task_delivery(first_dispatch);
      drop(callback_lease);
      assert_eq!(
        metrics().runnable_polls,
        HOST_TURN_RUNNABLE_BUDGET as u64,
        "the timer operation must occupy the last poll in the host-turn budget"
      );
      assert_eq!(
        state.pending.lock().unwrap_or_else(std::sync::PoisonError::into_inner).len(),
        1,
        "the timer poll must register its pending relay"
      );
      assert_eq!(
        metrics().queued_runnables,
        1,
        "the unpolled relay must be the sole runnable beyond the budget boundary"
      );

      let failed_dispatch = dispatch_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("budget exhaustion must publish a fresh relay turn");
      cancel_current_thread_task_dispatch(failed_dispatch.capability());
      let replacement_dispatch = dispatch_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("the first scheduler failure must publish one replacement");
      assert_ne!(replacement_dispatch, failed_dispatch);
      cancel_current_thread_task_dispatch(replacement_dispatch.capability());

      assert!(
        state.pending.lock().unwrap_or_else(std::sync::PoisonError::into_inner).is_empty(),
        "terminal queue destruction must retire the matching pending relay"
      );
      let cancelled_relays =
        state.cancelled_relays.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
      if schedule_state == RelayScheduleState::CallbackComplete {
        assert_eq!(
          cancelled_relays.len(),
          1,
          "a callback-complete relay must queue exactly one host cancellation"
        );
      } else {
        assert!(
          cancelled_relays.is_empty(),
          "cleanup must not overtake a schedule callback that has not returned"
        );
      }
      drop(cancelled_relays);
      assert_eq!(metrics().queued_runnables, 0);
      assert!(
        matches!(dispatch_rx.try_recv(), Err(mpsc::TryRecvError::Empty)),
        "terminal recovery must stay bounded after the replacement failure"
      );
      let task_result =
        timer_task.now_or_never().expect("the original timer operation must settle immediately");
      assert_eq!(
        task_result
          .expect_err("terminal recovery must cancel the pending timer operation")
          .to_string(),
        "async runtime stopped before the task completed"
      );

      unregister_current_thread_task_driver(driver_id);
      shutdown().expect("the isolated CurrentThread runtime must shut down cleanly");
      return;
    }

    for relay_state in ["awaiting", "complete"] {
      let output = std::process::Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg(
          "async_runtime::tests::timer_relay_terminal_drop_at_host_turn_budget_settles_without_later_work",
        )
        .arg("--nocapture")
        .env(CHILD_ENV, relay_state)
        .output()
        .expect("the timer relay regression subprocess must start");
      assert!(
        output.status.success(),
        "{relay_state} timer relay terminal-drop regression failed; status={:?}\nstdout={}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
      );
    }
  }

  #[test]
  fn native_thread_env_overrides_clamp_to_production_limits() {
    let shared = resolve(
      ResolvedRuntimeTarget::Native,
      &RuntimeEnv {
        worker_threads: Some("1000000".to_string()),
        max_blocking_threads: Some("1000000".to_string()),
        ..RuntimeEnv::default()
      },
    );
    assert_eq!(shared.worker_threads, max_async_runtime_worker_threads());
    assert_eq!(
      shared.max_blocking_tasks,
      max_async_runtime_worker_threads() - 1,
      "the shared cap must still reserve one runnable lane"
    );
  }

  #[test]
  fn native_defaults_respect_host_and_container_parallelism() {
    assert_eq!(native_default_parallelism(128, 2), 2);
    assert_eq!(native_default_parallelism(8, 16), 8);
    assert_eq!(native_default_parallelism(1, 1), 1);
    assert_eq!(native_default_parallelism(0, 0), 1);
  }

  #[test]
  fn shared_native_defaults_reserve_one_runnable_lane() {
    let resolved = resolve(ResolvedRuntimeTarget::Native, &env());
    assert_eq!(resolved.flavor, ResolvedRuntimeFlavor::MultiThread);
    assert_eq!(
      resolved.worker_threads,
      native_default_parallelism(num_cpus::get_physical(), num_cpus::get())
        .min(max_async_runtime_worker_threads())
        .max(2)
    );
    assert_eq!(
      resolved.max_blocking_tasks,
      resolved.worker_threads.saturating_sub(1).max(1),
      "blocking admission must preserve one runnable execution lane"
    );
    assert_eq!(resolved.park_deadline_ms, None);
  }

  #[test]
  fn shared_native_env_overrides_and_flavor_selection() {
    // The blocking default follows the RESOLVED worker count, then reserves
    // one lane: with workers overridden to 7 the blocking cap is 6.
    let resolved = resolve(
      ResolvedRuntimeTarget::Native,
      &RuntimeEnv { worker_threads: Some("7".to_string()), ..RuntimeEnv::default() },
    );
    assert_eq!((resolved.worker_threads, resolved.max_blocking_tasks), (7, 6));

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
        ResolvedRuntimeTarget::Native,
        &RuntimeEnv { runtime: Some(raw.to_string()), ..RuntimeEnv::default() },
      );
      assert_eq!(resolved.flavor, expected, "ROLLDOWN_RUNTIME={raw}");
    }
  }

  #[test]
  fn shared_multi_thread_one_worker_override_reports_effective_two_worker_minimum() {
    let resolved = resolve(
      ResolvedRuntimeTarget::Native,
      &RuntimeEnv {
        worker_threads: Some("1".to_string()),
        max_blocking_threads: Some("8".to_string()),
        ..RuntimeEnv::default()
      },
    );
    assert_eq!(resolved.flavor, ResolvedRuntimeFlavor::MultiThread);
    assert_eq!(
      (resolved.worker_threads, resolved.max_blocking_tasks),
      (2, 1),
      "the resolved snapshot must match the physical pool the controller will create"
    );
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
      ResolvedRuntimeTarget::Native,
      &RuntimeEnv { park_deadline_ms: Some("60000".to_string()), ..RuntimeEnv::default() },
    );
    assert_eq!(resolved.park_deadline_ms, Some(60000));
  }

  #[test]
  fn shared_wasi_defaults_keep_runtime_options_parity() {
    for target in [ResolvedRuntimeTarget::Wasi, ResolvedRuntimeTarget::WasiThreads] {
      let resolved = resolve(target, &env());
      assert_eq!(resolved.target, target);
      assert_eq!(
        resolved.flavor,
        ResolvedRuntimeFlavor::CurrentThread,
        "the shared wasm default flavor is CurrentThread"
      );
      assert_eq!(
        resolved.worker_threads, 1,
        "CurrentThread reporting must match its single physical execution lane"
      );
      assert_eq!(resolved.max_blocking_tasks, 1);

      // ROLLDOWN_WORKER_THREADS has never applied on wasm. An inherited
      // `ROLLDOWN_RUNTIME=multi` must be normalized before module init:
      // `configure` rejects MultiThread on every shared WebAssembly build,
      // and an expect panic while loading the addon is not an acceptable
      // configuration error.
      let overridden = resolve(
        target,
        &RuntimeEnv {
          runtime: Some("multi".to_string()),
          worker_threads: Some("9".to_string()),
          max_blocking_threads: Some("3".to_string()),
          ..RuntimeEnv::default()
        },
      );
      assert_eq!(overridden.flavor, ResolvedRuntimeFlavor::CurrentThread);
      assert_eq!(overridden.worker_threads, 1);
      assert_eq!(overridden.max_blocking_tasks, 1);
    }
  }

  /// Relay-eviction invariant: the relay must evict ONLY on host death, and
  /// the decision must be STRING-FREE. A rejected JS promise coerces to
  /// `GenericFailure` carrying the JS-controlled rejection string (pinned
  /// napi 3.10 error.rs), so message matching is forgeable by a live
  /// callback. The two authorities: the unforgeable `Closing` status, and
  /// the liveness probe.
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

    // A DEAD probe evicts regardless of status. Queue drain during env
    // teardown and env death before a JS promise settles both coincide with
    // the env being torn down, which the probe observes directly.
    assert!(should_evict_for_relay_error(Status::GenericFailure, false));
    assert!(should_evict_for_relay_error(Status::PendingException, false));

    // A LIVE host's failure takes the strike path no matter what the error
    // says. A live callback can reject with
    // `Promise.reject(new Error('oneshot canceled'))`, which coerces to
    // GenericFailure + "Error: oneshot canceled" and must not be mistaken for
    // environment teardown.
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

  #[test]
  fn cancellation_failures_share_the_three_strike_budget_without_cleanup_double_counting() {
    use std::sync::atomic::{AtomicU32, Ordering};

    use napi::Status;

    let failures = AtomicU32::new(0);
    for expected_strikes in 1..HOST_TIMER_MAX_TRANSIENT_FAILURES {
      let relay_health = HostTimerRelayHealth::default();
      assert_eq!(
        record_host_timer_failure(&failures, &relay_health, Status::GenericFailure, true),
        HostTimerFailureAction::Retry(expected_strikes),
      );
      assert_eq!(
        record_host_timer_failure(&failures, &relay_health, Status::GenericFailure, true),
        HostTimerFailureAction::Duplicate,
        "one relay failure must consume at most one strike"
      );
    }

    let cleanup_health = HostTimerRelayHealth::default();
    cleanup_health.set_cancellation_accounting(RelayCancellationAccounting::CleanupOnly);
    assert_eq!(cleanup_health.cancellation_accounting(), RelayCancellationAccounting::CleanupOnly);
    assert_eq!(
      failures.load(Ordering::SeqCst),
      HOST_TIMER_MAX_TRANSIENT_FAILURES - 1,
      "schedule-failure and eviction cleanup must not mutate host health"
    );

    let final_health = HostTimerRelayHealth::default();
    assert_eq!(
      record_host_timer_failure(&failures, &final_health, Status::GenericFailure, true),
      HostTimerFailureAction::EvictHostAfterStrikes(HOST_TIMER_MAX_TRANSIENT_FAILURES),
    );
  }

  #[test]
  fn a_same_relay_success_cannot_erase_its_recorded_failure() {
    use std::sync::atomic::{AtomicU32, Ordering};

    use napi::Status;

    let failures = AtomicU32::new(1);
    let healthy_relay = HostTimerRelayHealth::default();
    reset_host_timer_failures_after_success(&failures, &healthy_relay);
    assert_eq!(failures.load(Ordering::SeqCst), 0);

    failures.store(1, Ordering::SeqCst);
    let failed_relay = HostTimerRelayHealth::default();
    assert_eq!(
      record_host_timer_failure(&failures, &failed_relay, Status::GenericFailure, true),
      HostTimerFailureAction::Retry(2),
    );
    reset_host_timer_failures_after_success(&failures, &failed_relay);
    assert_eq!(
      failures.load(Ordering::SeqCst),
      2,
      "a later success from the same relay must not hide its cancellation failure"
    );
  }
}
