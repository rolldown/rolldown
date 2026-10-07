// The napi surface is reachable only from JS, so the unit-test binary sees it as dead code.
#![cfg_attr(test, allow(dead_code))]

use std::{future::Future, pin::Pin, ptr};

use napi::bindgen_prelude::Unknown;
use napi::bindgen_prelude::{
  AsyncRuntime, AsyncRuntimeRejection, AsyncRuntimeTask, register_async_runtime,
};
use napi_derive::napi;
use rolldown_utils::async_runtime::{
  CurrentThreadTaskDelivery, CurrentThreadTaskDriver, CurrentThreadTaskDriverId, RuntimeFlavor,
  RuntimeOptions, acknowledge_current_thread_task_delivery, begin_shutdown, configure,
  drive_current_thread_tasks, fail_current_thread_task_delivery, finish_shutdown,
  max_async_runtime_worker_threads, register_current_thread_task_driver,
  request_current_thread_task_drain, runtime_work_pending, shutdown, start, try_block_on_dyn,
  try_spawn, try_spawn_blocking, unregister_current_thread_task_driver,
};

struct RolldownAsyncRuntime;

/// Every async-runtime error reaches JavaScript as a plain reason string.
fn to_napi_error(error: impl std::fmt::Display) -> napi::Error {
  napi::Error::from_reason(error.to_string())
}

// SAFETY: `shutdown` closes admission, waits for the scheduler generation to quiesce,
// joins native workers and releases active resources. Independently, napi-rs permanently
// retains the native image once a module that registered this backend exported
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
      Err((error, task)) => Err(AsyncRuntimeRejection::new(task, to_napi_error(error))),
    }
  }

  fn block_on(&self, future: Pin<&mut dyn Future<Output = ()>>) -> napi::Result<()> {
    try_block_on_dyn(future).map_err(to_napi_error)
  }

  fn spawn_blocking(
    &self,
    work: Box<dyn FnOnce() + Send + 'static>,
  ) -> std::result::Result<(), AsyncRuntimeRejection<Box<dyn FnOnce() + Send + 'static>>> {
    // Same bounded blocking lane as Rolldown's own facade.
    match try_spawn_blocking(work) {
      Ok(handle) => {
        handle.detach();
        Ok(())
      }
      Err((error, work)) => Err(AsyncRuntimeRejection::new(work, to_napi_error(error))),
    }
  }

  fn start(&self) -> napi::Result<()> {
    start().map_err(to_napi_error)
  }

  fn shutdown(&self) -> napi::Result<()> {
    shutdown().map_err(to_napi_error)
  }

  // Two-phase teardown, as in napi-async-runtime's adapter. napi calls these only from its
  // wasm cleanup exports, so the JS event loop can turn between the phases: on
  // wasm32-wasip1-threads a running blocking closure may be waiting on the JS thread.
  fn begin_shutdown(&self) -> napi::Result<bool> {
    begin_shutdown().map_err(to_napi_error)
  }

  fn shutdown_work_pending(&self) -> bool {
    runtime_work_pending()
  }

  fn finish_shutdown(&self) -> napi::Result<()> {
    finish_shutdown().map_err(to_napi_error)
  }
}

#[napi(string_enum)]
#[derive(Clone, Copy)]
pub enum BindingRuntimeFlavor {
  CurrentThread,
  MultiThread,
}

impl From<RuntimeFlavor> for BindingRuntimeFlavor {
  fn from(value: RuntimeFlavor) -> Self {
    match value {
      RuntimeFlavor::CurrentThread => Self::CurrentThread,
      RuntimeFlavor::MultiThread => Self::MultiThread,
    }
  }
}

// See internal-docs/async-runtime/implementation.md, "Configuration".

/// Which target family this binding was compiled for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvedRuntimeTarget {
  Native,
  /// `wasm32-wasip1`: threadless wasm (no atomics).
  Wasi,
  /// `wasm32-wasip1-threads`: wasm with real OS threads (atomics).
  WasiThreads,
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
}

impl RuntimeEnv {
  fn from_process() -> Self {
    Self {
      runtime: std::env::var("ROLLDOWN_RUNTIME").ok(),
      worker_threads: std::env::var("ROLLDOWN_WORKER_THREADS").ok(),
      max_blocking_threads: std::env::var("ROLLDOWN_MAX_BLOCKING_THREADS").ok(),
    }
  }
}

/// The effective values the runtime is built from. CurrentThread is normalized to one
/// worker, MultiThread to a minimum of two, before either reaches the controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedRuntimeConfig {
  pub target: ResolvedRuntimeTarget,
  pub flavor: RuntimeFlavor,
  pub worker_threads: usize,
  pub max_blocking_tasks: usize,
}

const fn compiled_target() -> ResolvedRuntimeTarget {
  // See build.rs: only the cargo TARGET distinguishes the two WASI targets.
  if cfg!(not(target_family = "wasm")) {
    ResolvedRuntimeTarget::Native
  } else if cfg!(rolldown_wasi_threads) {
    ResolvedRuntimeTarget::WasiThreads
  } else {
    ResolvedRuntimeTarget::Wasi
  }
}

/// Parse a raw `ROLLDOWN_RUNTIME` value; unknown / unset values keep
/// `default`, Rolldown's per-target flavor chosen in `resolve_runtime_config_for`.
fn resolve_runtime_flavor(raw: Option<&str>, default: RuntimeFlavor) -> RuntimeFlavor {
  match raw {
    Some("current" | "current-thread" | "single" | "single-thread") => RuntimeFlavor::CurrentThread,
    Some("multi" | "multi-thread") => RuntimeFlavor::MultiThread,
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
  flavor: RuntimeFlavor,
  worker_threads: usize,
  requested: usize,
) -> usize {
  match flavor {
    RuntimeFlavor::CurrentThread => 1,
    RuntimeFlavor::MultiThread => requested.min(worker_threads.saturating_sub(1).max(1)),
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
  let threads = matches!(target, ResolvedRuntimeTarget::WasiThreads);
  // Each artifact has one flavor except wasm32-wasip1-threads, where `ROLLDOWN_RUNTIME=single`
  // selects CurrentThread over the MultiThread default. Native is forced to MultiThread (no
  // host-driven timers back `watch()` on CurrentThread) and threadless wasm32-wasip1 to
  // CurrentThread (no threads), so an inherited `ROLLDOWN_RUNTIME` cannot reach `configure`.
  //
  // wasm32-wasip1-threads clamps `ROLLDOWN_WORKER_THREADS` to [2, 4], default 2: every thread
  // allocates through wasi-libc dlmalloc, whose global lock spins with `sched_yield`, so wider
  // pools build slower. MultiThread is safe there only with napi's heap-sync workaround:
  // https://github.com/napi-rs/napi-rs/blob/main/cli/docs/wasi-heap-sync-design.md
  let default_flavor =
    if native || threads { RuntimeFlavor::MultiThread } else { RuntimeFlavor::CurrentThread };
  let flavor = if threads {
    resolve_runtime_flavor(env.runtime.as_deref(), default_flavor)
  } else {
    default_flavor
  };
  let requested_worker_threads = if threads {
    resolve_thread_count(env.worker_threads.clone(), 2, max_async_runtime_worker_threads().min(4))
  } else if native {
    resolve_thread_count(
      env.worker_threads.clone(),
      detected_native_parallelism(),
      max_async_runtime_worker_threads(),
    )
  } else {
    // `ROLLDOWN_WORKER_THREADS` does not apply on threadless wasm, and the flavor
    // above is forced to CurrentThread there, so this value is never read.
    1
  };
  let worker_threads = match flavor {
    RuntimeFlavor::CurrentThread => 1,
    RuntimeFlavor::MultiThread => requested_worker_threads.max(2),
  };
  let requested_blocking_tasks =
    resolve_thread_count(env.max_blocking_threads.clone(), worker_threads, worker_threads);
  let max_blocking_tasks =
    clamp_shared_blocking_tasks(flavor, worker_threads, requested_blocking_tasks);
  ResolvedRuntimeConfig { target, flavor, worker_threads, max_blocking_tasks }
}

/// The per-process resolved runtime-config snapshot. lib.rs `init` forces it at module
/// load, when the WASI loader sizes its async work pool, so a later env change cannot make
/// the report diverge from the pool that already runs.
pub fn resolved_runtime_config() -> &'static ResolvedRuntimeConfig {
  static RESOLVED_RUNTIME_CONFIG: std::sync::OnceLock<ResolvedRuntimeConfig> =
    std::sync::OnceLock::new();
  RESOLVED_RUNTIME_CONFIG
    .get_or_init(|| resolve_runtime_config_for(compiled_target(), &RuntimeEnv::from_process()))
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
/// Reserve a CurrentThread host registration capability. The returned words
/// must be passed back to exactly one host registration call.
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
  std::panic::catch_unwind(std::panic::AssertUnwindSafe(operation))
    .map_err(rolldown_std_utils::discard_panic_payload)
    .ok()
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
    let status = self.call_threadsafe_function_with(data, |threadsafe_function, data| unsafe {
      napi::sys::napi_call_threadsafe_function(
        threadsafe_function,
        data,
        napi::sys::ThreadsafeFunctionCallMode::nonblocking,
      )
    });
    // `None` (host not live) and a non-ok status both mean Node-API did not take the payload.
    if status != Some(napi::sys::Status::napi_ok) {
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
/// Return the CurrentThread task-host ABI version this binding implements.
/// Check it before calling either async-runtime host registration.
pub fn get_current_thread_task_host_contract_version() -> u32 {
  CURRENT_THREAD_TASK_HOST_CONTRACT_VERSION
}

#[napi]
/// Return whether the given CurrentThread task- or timer-host registration is
/// still live. A registration already evicted natively reads false.
pub fn is_current_thread_host_registration_active(
  registration_high: u32,
  registration_low: u32,
) -> bool {
  let id = BindingHostRegistration::id(registration_high, registration_low);
  registered_current_thread_task_host(id).is_some_and(|inner| inner.is_live())
    || timer_host_registrations().contains(&id)
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

#[napi(ts_args_type = "registrationHigh: number, registrationLow: number, dispatch?: never")]
/// Install the native host turn that polls CurrentThread runnables. Call it
/// once per importing environment; passing a JavaScript callback throws.
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
/// Evict one host installed by `registerCurrentThreadTaskHost`.
pub fn unregister_current_thread_task_host(registration_high: u32, registration_low: u32) {
  let id = BindingHostRegistration::id(registration_high, registration_low);
  release_host_registration_id(id);
  if let Some(inner) = registered_current_thread_task_host(id) {
    inner.rollback();
  }
}

/// Live `registerTimerHost` registrations. No timer driver backs them: the only
/// `sleep_until` caller is the watch debounce, and no CurrentThread artifact runs
/// `watch()` (native is MultiThread-only and every WASI artifact rejects watch).
/// The cli loaders still register a timer host on every CurrentThread artifact, so
/// the registration is accepted and reported live, while a CurrentThread
/// `sleep_until` panics with the scheduler's "no live timer driver" message.
static TIMER_HOST_REGISTRATIONS: std::sync::LazyLock<std::sync::Mutex<rustc_hash::FxHashSet<u64>>> =
  std::sync::LazyLock::new(|| std::sync::Mutex::new(rustc_hash::FxHashSet::default()));

fn timer_host_registrations() -> std::sync::MutexGuard<'static, rustc_hash::FxHashSet<u64>> {
  TIMER_HOST_REGISTRATIONS.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn forget_timer_host_registration(id: u64) {
  timer_host_registrations().remove(&id);
}

#[napi(
  ts_args_type = "registrationHigh: number, registrationLow: number, schedule: (id: number, ms: number) => Promise<void>, cancel: (id: number) => void"
)]
/// Accept the host timer callbacks of the CurrentThread host contract. They are
/// never called: CurrentThread runs no timers.
pub fn register_timer_host(
  env: &napi::Env,
  registration_high: u32,
  registration_low: u32,
  _schedule: Unknown<'_>,
  _cancel: Unknown<'_>,
) -> napi::Result<()> {
  let id = claim_host_registration_id(registration_high, registration_low)?;
  timer_host_registrations().insert(id);
  install_cleanup_hook_or_rollback(
    || env.add_env_cleanup_hook(id, forget_timer_host_registration),
    || forget_timer_host_registration(id),
  )
}

#[napi]
/// Evict one registration made by `registerTimerHost`.
pub fn unregister_timer_host(registration_high: u32, registration_low: u32) {
  let id = BindingHostRegistration::id(registration_high, registration_low);
  release_host_registration_id(id);
  forget_timer_host_registration(id);
}

#[napi_derive::module_init]
fn install_async_runtime_backend() {
  // The same resolved snapshot `get_runtime_capabilities` reports from.
  let resolved = resolved_runtime_config();
  let options = RuntimeOptions {
    flavor: resolved.flavor,
    worker_threads: resolved.worker_threads,
    max_blocking_tasks: resolved.max_blocking_tasks,
    park_deadline: None,
    drain_linger: std::time::Duration::from_micros(
      rolldown_utils::async_runtime::DEFAULT_DRAIN_LINGER_MICROS,
    ),
    // The shared crate defaults to a neutral prefix; pin Rolldown's thread names.
    thread_name_prefix: "rolldown-runtime".to_string(),
  };
  configure(options).expect("Failed to configure the Rolldown async runtime");
  register_async_runtime(RolldownAsyncRuntime);
}

/// Stop the real shared scheduler so the next N-API future submission is
/// rejected. Exported only by the dedicated async-runtime integration build.
#[cfg(all(feature = "runtime-submission-failure-test", not(target_family = "wasm")))]
#[napi(js_name = "__rolldownTestStopAsyncRuntime")]
pub fn stop_async_runtime_for_submission_failure_test() -> napi::Result<()> {
  shutdown().map_err(to_napi_error)
}

/// Restart the scheduler after `__rolldownTestStopAsyncRuntime`.
#[cfg(all(feature = "runtime-submission-failure-test", not(target_family = "wasm")))]
#[napi(js_name = "__rolldownTestStartAsyncRuntime")]
pub fn start_async_runtime_for_submission_failure_test() -> napi::Result<()> {
  start().map_err(to_napi_error)
}

/// What this binding is -- flavor, target -- and the capabilities that follow
/// from it. Frozen at binding load; never re-read from the environment.
// Independent capability flags on a napi object consumed from JS, not state to model.
#[expect(clippy::struct_excessive_bools)]
#[napi(object)]
pub struct BindingRuntimeCapabilities {
  /// The executor flavor in effect.
  pub flavor: BindingRuntimeFlavor,
  /// The compile target: 'native', 'wasi' (threadless `wasm32-wasip1`) or
  /// 'wasi-threads' (`wasm32-wasip1-threads`).
  #[napi(ts_type = "'native' | 'wasi' | 'wasi-threads'")]
  pub target: String,
  /// The binding is a WebAssembly/WASI artifact (`target !== 'native'`).
  pub wasi: bool,
  /// The scheduler spreads its work over several executor threads (`flavor
  /// === 'MultiThread'`). Native is always MultiThread; its data-parallel
  /// work runs on the executor's own Rayon pool, where spawned futures are
  /// polled. `false` only on threadless WASI and on threaded WASI under
  /// `ROLLDOWN_RUNTIME=single`.
  pub threads: bool,
  /// Dev mode is supported by this runtime: true on MultiThread, false on
  /// CurrentThread.
  pub dev_supported: bool,
  /// Watch mode is supported by this artifact: true on native, false on every
  /// wasm artifact.
  pub watch_supported: bool,
}

#[napi]
/// Report the loaded binding's runtime capabilities.
pub fn get_runtime_capabilities() -> BindingRuntimeCapabilities {
  let resolved = resolved_runtime_config();
  let target = match resolved.target {
    ResolvedRuntimeTarget::Native => "native",
    ResolvedRuntimeTarget::Wasi => "wasi",
    ResolvedRuntimeTarget::WasiThreads => "wasi-threads",
  };
  let wasi = !matches!(resolved.target, ResolvedRuntimeTarget::Native);
  let flavor: BindingRuntimeFlavor = resolved.flavor.into();
  let threads = matches!(flavor, BindingRuntimeFlavor::MultiThread);
  BindingRuntimeCapabilities {
    flavor,
    target: target.to_string(),
    wasi,
    threads,
    dev_supported: threads,
    // Static per artifact: the capability contract must not depend on import
    // order or registration state.
    watch_supported: !wasi,
  }
}

#[cfg(test)]
mod tests {
  use rolldown_utils::async_runtime::max_async_runtime_worker_threads;

  use super::{
    BindingHostRegistration, ResolvedRuntimeTarget, RuntimeEnv, RuntimeFlavor,
    claim_host_registration_id, native_default_parallelism,
    reserve_current_thread_host_registration, resolve_runtime_config_for,
    unregister_current_thread_task_host,
  };
  use super::{
    NATIVE_TASK_HOST_AFTER_DRIVE_TEST_HOOK, NATIVE_TASK_HOST_AFTER_PAYLOAD_DROP_TEST_HOOK,
    NativeCurrentThreadTaskHostInner, NativeCurrentThreadTaskHostPayload, RolldownAsyncRuntime,
    call_native_current_thread_task_host,
  };

  fn env() -> RuntimeEnv {
    RuntimeEnv::default()
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
    assert_eq!(resolved.flavor, RuntimeFlavor::MultiThread);
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

    // Native runs MultiThread only: an inherited `ROLLDOWN_RUNTIME` is ignored.
    for raw in ["single", "current-thread", "multi", "turbo"] {
      let resolved = resolve(
        ResolvedRuntimeTarget::Native,
        &RuntimeEnv { runtime: Some(raw.to_string()), ..RuntimeEnv::default() },
      );
      assert_eq!(resolved.flavor, RuntimeFlavor::MultiThread, "ROLLDOWN_RUNTIME={raw}");
    }

    for (raw, expected) in [
      ("single", RuntimeFlavor::CurrentThread),
      ("single-thread", RuntimeFlavor::CurrentThread),
      ("current", RuntimeFlavor::CurrentThread),
      ("current-thread", RuntimeFlavor::CurrentThread),
      ("multi", RuntimeFlavor::MultiThread),
      ("multi-thread", RuntimeFlavor::MultiThread),
      // Unknown values keep the per-target default (MultiThread on wasm32-wasip1-threads).
      ("turbo", RuntimeFlavor::MultiThread),
    ] {
      let resolved = resolve(
        ResolvedRuntimeTarget::WasiThreads,
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
    assert_eq!(resolved.flavor, RuntimeFlavor::MultiThread);
    assert_eq!(
      (resolved.worker_threads, resolved.max_blocking_tasks),
      (2, 1),
      "the resolved snapshot must match the physical pool the controller will create"
    );
  }

  #[test]
  fn shared_wasi_defaults_follow_each_artifact_thread_support() {
    // Threadless wasm32-wasip1 has one execution lane: CurrentThread.
    let threadless = resolve(ResolvedRuntimeTarget::Wasi, &env());
    assert_eq!(threadless.target, ResolvedRuntimeTarget::Wasi);
    assert_eq!(
      (threadless.flavor, threadless.worker_threads, threadless.max_blocking_tasks),
      (RuntimeFlavor::CurrentThread, 1, 1),
      "threadless wasm defaults to CurrentThread and reports its single lane"
    );

    // wasm32-wasip1-threads defaults to MultiThread with two workers; blocking
    // admission keeps one runnable lane.
    let threaded = resolve(ResolvedRuntimeTarget::WasiThreads, &env());
    assert_eq!(threaded.target, ResolvedRuntimeTarget::WasiThreads);
    assert_eq!(
      (threaded.flavor, threaded.worker_threads, threaded.max_blocking_tasks),
      (RuntimeFlavor::MultiThread, 2, 1),
      "threaded wasm defaults to MultiThread with two workers"
    );
  }

  #[test]
  fn wasi_single_runtime_env_selects_current_thread_on_both_targets() {
    // `ROLLDOWN_RUNTIME=single` is the opt-out on wasm32-wasip1-threads and keeps the
    // same one-lane shape on threadless wasm32-wasip1; a worker count is ignored.
    for target in [ResolvedRuntimeTarget::Wasi, ResolvedRuntimeTarget::WasiThreads] {
      for raw in ["single", "single-thread", "current", "current-thread"] {
        let single = resolve(
          target,
          &RuntimeEnv {
            runtime: Some(raw.to_string()),
            worker_threads: Some("9".to_string()),
            ..RuntimeEnv::default()
          },
        );
        assert_eq!(
          (single.flavor, single.worker_threads, single.max_blocking_tasks),
          (RuntimeFlavor::CurrentThread, 1, 1),
          "{target:?} ROLLDOWN_RUNTIME={raw}"
        );
      }
    }
  }

  #[test]
  fn threadless_wasi_normalizes_an_inherited_multi_thread_request() {
    // `ROLLDOWN_WORKER_THREADS` does not apply on threadless wasm, and an inherited
    // `ROLLDOWN_RUNTIME=multi` must be normalized before module init: threadless
    // wasm32-wasip1 `configure` would reject it and panic while loading the addon.
    let overridden = resolve(
      ResolvedRuntimeTarget::Wasi,
      &RuntimeEnv {
        runtime: Some("multi".to_string()),
        worker_threads: Some("9".to_string()),
        max_blocking_threads: Some("3".to_string()),
      },
    );
    assert_eq!(overridden.flavor, RuntimeFlavor::CurrentThread);
    assert_eq!(overridden.worker_threads, 1);
    assert_eq!(overridden.max_blocking_tasks, 1);
  }

  #[test]
  fn threaded_wasi_runs_multi_thread_by_default_within_two_to_four_workers() {
    // `ROLLDOWN_RUNTIME` unset, `multi`, or unknown all give the MultiThread default.
    for runtime in [None, Some("multi"), Some("multi-thread"), Some("turbo")] {
      let resolve_threads = |worker_threads: Option<&str>, max_blocking_threads: Option<&str>| {
        resolve(
          ResolvedRuntimeTarget::WasiThreads,
          &RuntimeEnv {
            runtime: runtime.map(str::to_string),
            worker_threads: worker_threads.map(str::to_string),
            max_blocking_threads: max_blocking_threads.map(str::to_string),
          },
        )
      };

      // Clamped to the 4-worker ceiling; blocking admission keeps one runnable lane.
      let wide = resolve_threads(Some("9"), Some("3"));
      assert_eq!(wide.target, ResolvedRuntimeTarget::WasiThreads);
      assert_eq!(wide.flavor, RuntimeFlavor::MultiThread, "ROLLDOWN_RUNTIME={runtime:?}");
      assert_eq!((wide.worker_threads, wide.max_blocking_tasks), (4, 3));

      // Unset defaults to two workers, the MultiThread minimum.
      let unset = resolve_threads(None, None);
      assert_eq!(
        (unset.flavor, unset.worker_threads, unset.max_blocking_tasks),
        (RuntimeFlavor::MultiThread, 2, 1),
        "ROLLDOWN_RUNTIME={runtime:?}"
      );

      // Values inside the range pass through; below it rises to the minimum.
      assert_eq!(resolve_threads(Some("3"), None).worker_threads, 3);
      assert_eq!(resolve_threads(Some("1"), None).worker_threads, 2);
    }
  }
}
