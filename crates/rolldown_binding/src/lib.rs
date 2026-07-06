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

#[cfg(not(any(feature = "tokio-runtime", feature = "async-runtime")))]
compile_error!(
  "rolldown_binding requires at least one async runtime feature: enable `tokio-runtime` or `async-runtime`"
);

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
  use std::{
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    sync::{Condvar, Mutex},
  };

  use napi::{Error, Status};

  #[derive(Clone, Copy, Debug, PartialEq, Eq)]
  enum Lifecycle {
    Stopped,
    Starting,
    Running,
    Stopping,
    ShutdownFailed,
  }

  struct State {
    lifecycle: Lifecycle,
    lease_owner_count: u32,
    manual_owner_count: u32,
    abandoned_lease_owner: bool,
  }

  impl State {
    fn owner_count(&self) -> u64 {
      u64::from(self.lease_owner_count) + u64::from(self.manual_owner_count)
    }
  }

  #[derive(Clone, Copy, Debug, PartialEq, Eq)]
  enum OwnerKind {
    Lease,
    Manual,
  }

  #[derive(Clone, Copy)]
  enum ReleaseKind {
    Explicit,
    Finalizer,
  }

  pub struct Manager {
    state: Mutex<State>,
    transition: Condvar,
  }

  impl Manager {
    pub const fn new() -> Self {
      Self {
        state: Mutex::new(State {
          lifecycle: Lifecycle::Stopped,
          lease_owner_count: 0,
          manual_owner_count: 0,
          abandoned_lease_owner: false,
        }),
        transition: Condvar::new(),
      }
    }

    pub fn acquire_lease(
      &self,
      is_cancelled: impl Fn() -> bool,
      recover_shutdown: impl FnOnce() -> napi::Result<()>,
      start: impl FnOnce() -> napi::Result<()>,
    ) -> napi::Result<()> {
      self.acquire(OwnerKind::Lease, is_cancelled, recover_shutdown, start)
    }

    pub fn acquire_manual(
      &self,
      recover_shutdown: impl FnOnce() -> napi::Result<()>,
      start: impl FnOnce() -> napi::Result<()>,
    ) -> napi::Result<()> {
      self.acquire(OwnerKind::Manual, || false, recover_shutdown, start)
    }

    fn acquire(
      &self,
      owner_kind: OwnerKind,
      is_cancelled: impl Fn() -> bool,
      recover_shutdown: impl FnOnce() -> napi::Result<()>,
      start: impl FnOnce() -> napi::Result<()>,
    ) -> napi::Result<()> {
      let mut recover_shutdown = Some(recover_shutdown);
      let mut start = Some(start);
      loop {
        let mut state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        while matches!(state.lifecycle, Lifecycle::Starting | Lifecycle::Stopping) {
          if is_cancelled() {
            return Err(cancelled_error());
          }
          state = self.transition.wait(state).unwrap_or_else(std::sync::PoisonError::into_inner);
        }
        if is_cancelled() {
          return Err(cancelled_error());
        }

        let recovering_shutdown = match state.lifecycle {
          Lifecycle::Running => {
            increment_owner(&mut state, owner_kind)?;
            return Ok(());
          }
          Lifecycle::Stopped => {
            debug_assert_eq!(state.owner_count(), 0);
            state.lifecycle = Lifecycle::Starting;
            false
          }
          Lifecycle::ShutdownFailed => {
            if !state.abandoned_lease_owner {
              return Err(explicit_shutdown_retry_error());
            }
            debug_assert_eq!(state.lease_owner_count, 1);
            debug_assert_eq!(state.manual_owner_count, 0);
            state.lifecycle = Lifecycle::Stopping;
            true
          }
          Lifecycle::Starting | Lifecycle::Stopping => unreachable!(),
        };
        drop(state);

        let result = if recovering_shutdown {
          catch_unwind(AssertUnwindSafe(
            recover_shutdown
              .take()
              .expect("runtime shutdown recovery closure is used at most once"),
          ))
        } else {
          catch_unwind(AssertUnwindSafe(
            start.take().expect("runtime start closure is used at most once"),
          ))
        };
        let mut state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if recovering_shutdown {
          match &result {
            Ok(Ok(())) => {
              state.lifecycle = Lifecycle::Stopped;
              state.abandoned_lease_owner = false;
              decrement_owner(&mut state, OwnerKind::Lease);
              debug_assert_eq!(state.owner_count(), 0);
            }
            Ok(Err(_)) | Err(_) => {
              state.lifecycle = Lifecycle::ShutdownFailed;
              debug_assert!(state.abandoned_lease_owner);
              debug_assert_eq!(state.owner_count(), 1);
            }
          }
          self.transition.notify_all();
          match result {
            Ok(Ok(())) => continue,
            Ok(Err(error)) => return Err(error),
            Err(payload) => resume_unwind(payload),
          }
        }

        match &result {
          Ok(Ok(())) => {
            state.lifecycle = Lifecycle::Running;
            increment_owner(&mut state, owner_kind)
              .expect("the first async runtime owner cannot overflow");
          }
          Ok(Err(_)) | Err(_) => {
            state.lifecycle = Lifecycle::Stopped;
            debug_assert_eq!(state.owner_count(), 0);
          }
        }
        self.transition.notify_all();
        return match result {
          Ok(result) => result,
          Err(payload) => resume_unwind(payload),
        };
      }
    }

    pub fn release_lease(&self, shutdown: impl FnOnce() -> napi::Result<()>) -> napi::Result<()> {
      self.release(OwnerKind::Lease, ReleaseKind::Explicit, shutdown)
    }

    pub fn release_lease_from_finalizer(&self, shutdown: impl FnOnce() -> napi::Result<()>) {
      contain_panic(|| {
        let _ = self.release(OwnerKind::Lease, ReleaseKind::Finalizer, shutdown);
      });
    }

    pub fn release_manual(&self, shutdown: impl FnOnce() -> napi::Result<()>) -> napi::Result<()> {
      self.release(OwnerKind::Manual, ReleaseKind::Explicit, shutdown)
    }

    fn release(
      &self,
      owner_kind: OwnerKind,
      release_kind: ReleaseKind,
      shutdown: impl FnOnce() -> napi::Result<()>,
    ) -> napi::Result<()> {
      let mut state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
      while matches!(state.lifecycle, Lifecycle::Starting | Lifecycle::Stopping) {
        state = self.transition.wait(state).unwrap_or_else(std::sync::PoisonError::into_inner);
      }
      let owner_count = match owner_kind {
        OwnerKind::Lease => state.lease_owner_count,
        OwnerKind::Manual => state.manual_owner_count,
      };
      match owner_count {
        0 => return Ok(()),
        1 => {
          debug_assert!(matches!(state.lifecycle, Lifecycle::Running | Lifecycle::ShutdownFailed));
          if state.owner_count() == 1 {
            state.lifecycle = Lifecycle::Stopping;
          } else {
            decrement_owner(&mut state, owner_kind);
            return Ok(());
          }
        }
        _ => {
          decrement_owner(&mut state, owner_kind);
          return Ok(());
        }
      }
      drop(state);

      let result = catch_unwind(AssertUnwindSafe(shutdown));
      let mut state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
      match &result {
        Ok(Ok(())) => {
          state.lifecycle = Lifecycle::Stopped;
          state.abandoned_lease_owner = false;
          decrement_owner(&mut state, owner_kind);
          debug_assert_eq!(state.owner_count(), 0);
        }
        Ok(Err(_)) | Err(_) => {
          state.lifecycle = Lifecycle::ShutdownFailed;
          if matches!(release_kind, ReleaseKind::Finalizer) {
            debug_assert_eq!(owner_kind, OwnerKind::Lease);
            state.abandoned_lease_owner = true;
          }
          debug_assert_eq!(state.owner_count(), 1);
        }
      }
      self.transition.notify_all();
      match result {
        Ok(result) => result,
        Err(payload) => resume_unwind(payload),
      }
    }

    pub fn notify_waiters(&self) {
      // Synchronize with the condition-variable wait handoff. Cancellation
      // lives in the caller's atomic flag, so notifying without taking this
      // mutex could race between the waiter's predicate check and its wait,
      // losing the only teardown wake.
      drop(self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner));
      self.transition.notify_all();
    }

    #[cfg(test)]
    fn owner_count(&self) -> u64 {
      self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner).owner_count()
    }

    #[cfg(test)]
    fn owner_counts(&self) -> (u32, u32) {
      let state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
      (state.lease_owner_count, state.manual_owner_count)
    }

    #[cfg(test)]
    fn has_abandoned_lease_owner(&self) -> bool {
      self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner).abandoned_lease_owner
    }
  }

  fn increment_owner(state: &mut State, owner_kind: OwnerKind) -> napi::Result<()> {
    if state.owner_count() >= u64::from(u32::MAX) {
      return Err(Error::new(Status::GenericFailure, "Async runtime owner count overflowed"));
    }
    let owner_count = match owner_kind {
      OwnerKind::Lease => &mut state.lease_owner_count,
      OwnerKind::Manual => &mut state.manual_owner_count,
    };
    *owner_count = owner_count
      .checked_add(1)
      .ok_or_else(|| Error::new(Status::GenericFailure, "Async runtime owner count overflowed"))?;
    Ok(())
  }

  fn decrement_owner(state: &mut State, owner_kind: OwnerKind) {
    let owner_count = match owner_kind {
      OwnerKind::Lease => &mut state.lease_owner_count,
      OwnerKind::Manual => &mut state.manual_owner_count,
    };
    *owner_count -= 1;
  }

  fn cancelled_error() -> napi::Error {
    Error::new(Status::Cancelled, "Async runtime acquisition was cancelled")
  }

  fn explicit_shutdown_retry_error() -> napi::Error {
    Error::new(
      Status::GenericFailure,
      "Async runtime shutdown previously failed; the retaining owner must retry release",
    )
  }

  fn contain_panic(action: impl FnOnce()) {
    if let Err(payload) = catch_unwind(AssertUnwindSafe(action))
      && let Err(nested_payload) = catch_unwind(AssertUnwindSafe(|| drop(payload)))
    {
      std::mem::forget(nested_payload);
    }
  }

  #[cfg(test)]
  mod tests {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::sync::{
      Arc, Barrier,
      atomic::{AtomicBool, AtomicUsize, Ordering},
      mpsc,
    };

    use super::Manager;

    #[test]
    fn every_owner_explicitly_starts_and_stops_the_runtime() {
      let manager = Manager::new();
      let start_calls = AtomicUsize::new(0);
      let shutdown_calls = AtomicUsize::new(0);

      manager
        .acquire_lease(
          || false,
          || unreachable!(),
          || {
            start_calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
          },
        )
        .unwrap();
      manager.acquire_lease(|| false, || unreachable!(), || unreachable!()).unwrap();
      assert_eq!(manager.owner_count(), 2);
      assert_eq!(start_calls.load(Ordering::SeqCst), 1);

      manager.release_lease(|| unreachable!()).unwrap();
      assert_eq!(manager.owner_count(), 1);
      manager
        .release_lease(|| {
          shutdown_calls.fetch_add(1, Ordering::SeqCst);
          Ok(())
        })
        .unwrap();
      assert_eq!(manager.owner_count(), 0);
      assert_eq!(shutdown_calls.load(Ordering::SeqCst), 1);

      manager
        .acquire_lease(
          || false,
          || unreachable!(),
          || {
            start_calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
          },
        )
        .unwrap();
      assert_eq!(manager.owner_count(), 1);
      assert_eq!(start_calls.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn start_and_shutdown_run_without_the_manager_mutex() {
      let manager = Manager::new();
      manager
        .acquire_lease(
          || false,
          || unreachable!(),
          || {
            assert_eq!(manager.owner_count(), 0);
            Ok(())
          },
        )
        .unwrap();
      manager
        .release_lease(|| {
          assert_eq!(manager.owner_count(), 1);
          Ok(())
        })
        .unwrap();
    }

    #[test]
    fn concurrent_acquisitions_share_one_start_transition() {
      let manager = Arc::new(Manager::new());
      let start_calls = Arc::new(AtomicUsize::new(0));
      let start_entered = Arc::new(Barrier::new(2));
      let release_start = Arc::new(Barrier::new(2));

      let first = {
        let manager = Arc::clone(&manager);
        let start_calls = Arc::clone(&start_calls);
        let start_entered = Arc::clone(&start_entered);
        let release_start = Arc::clone(&release_start);
        std::thread::spawn(move || {
          manager
            .acquire_lease(
              || false,
              || unreachable!(),
              || {
                start_calls.fetch_add(1, Ordering::SeqCst);
                start_entered.wait();
                release_start.wait();
                Ok(())
              },
            )
            .unwrap();
        })
      };
      start_entered.wait();

      let second = {
        let manager = Arc::clone(&manager);
        std::thread::spawn(move || {
          manager.acquire_lease(|| false, || unreachable!(), || unreachable!()).unwrap();
        })
      };
      release_start.wait();
      first.join().unwrap();
      second.join().unwrap();

      assert_eq!(start_calls.load(Ordering::SeqCst), 1);
      assert_eq!(manager.owner_count(), 2);
    }

    #[test]
    fn cancelled_waiter_does_not_acquire_after_another_start() {
      let manager = Arc::new(Manager::new());
      let cancelled = Arc::new(AtomicBool::new(false));
      let (start_entered_tx, start_entered_rx) = mpsc::channel();
      let (release_start_tx, release_start_rx) = mpsc::channel();

      let first = {
        let manager = Arc::clone(&manager);
        std::thread::spawn(move || {
          manager
            .acquire_lease(
              || false,
              || unreachable!(),
              || {
                start_entered_tx.send(()).unwrap();
                release_start_rx.recv().unwrap();
                Ok(())
              },
            )
            .unwrap();
        })
      };
      start_entered_rx.recv().unwrap();

      let second = {
        let manager = Arc::clone(&manager);
        let cancelled = Arc::clone(&cancelled);
        std::thread::spawn(move || {
          manager.acquire_lease(
            || cancelled.load(Ordering::SeqCst),
            || unreachable!(),
            || unreachable!(),
          )
        })
      };
      cancelled.store(true, Ordering::SeqCst);
      manager.notify_waiters();
      release_start_tx.send(()).unwrap();
      first.join().unwrap();

      let error = second.join().unwrap().unwrap_err();
      assert_eq!(error.status, napi::Status::Cancelled);
      assert_eq!(manager.owner_count(), 1);
    }

    #[test]
    fn failed_start_and_explicit_shutdown_preserve_ownership() {
      let stopped = Manager::new();
      let start_error = stopped
        .acquire_lease(
          || false,
          || unreachable!(),
          || Err(napi::Error::from_reason("start failed")),
        )
        .unwrap_err();
      assert_eq!(start_error.reason, "start failed");
      assert_eq!(stopped.owner_count(), 0);
      stopped.acquire_lease(|| false, || unreachable!(), || Ok(())).unwrap();
      assert_eq!(stopped.owner_count(), 1);

      let shutdown_error =
        stopped.release_lease(|| Err(napi::Error::from_reason("shutdown failed"))).unwrap_err();
      assert_eq!(shutdown_error.reason, "shutdown failed");
      assert_eq!(stopped.owner_count(), 1);
      assert!(!stopped.has_abandoned_lease_owner());

      let acquire_error =
        stopped.acquire_lease(|| false, || unreachable!(), || unreachable!()).unwrap_err();
      assert_eq!(
        acquire_error.reason,
        "Async runtime shutdown previously failed; the retaining owner must retry release"
      );
      assert_eq!(stopped.owner_count(), 1);

      stopped.release_lease(|| Ok(())).unwrap();
      assert_eq!(stopped.owner_count(), 0);
    }

    #[test]
    fn finalizer_failure_is_recovered_by_the_next_acquisition() {
      let manager = Manager::new();
      manager.acquire_lease(|| false, || unreachable!(), || Ok(())).unwrap();

      manager.release_lease_from_finalizer(|| {
        Err(napi::Error::from_reason("finalizer shutdown failed"))
      });
      assert_eq!(manager.owner_counts(), (1, 0));
      assert!(manager.has_abandoned_lease_owner());

      let recovery_calls = AtomicUsize::new(0);
      let start_calls = AtomicUsize::new(0);
      manager
        .acquire_lease(
          || false,
          || {
            assert_eq!(manager.owner_counts(), (1, 0));
            recovery_calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
          },
          || {
            assert_eq!(manager.owner_counts(), (0, 0));
            start_calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
          },
        )
        .unwrap();

      assert_eq!(recovery_calls.load(Ordering::SeqCst), 1);
      assert_eq!(start_calls.load(Ordering::SeqCst), 1);
      assert_eq!(manager.owner_counts(), (1, 0));
      assert!(!manager.has_abandoned_lease_owner());
      manager.release_lease(|| Ok(())).unwrap();
    }

    #[test]
    fn failed_finalizer_recovery_rejects_without_stranding_another_owner() {
      let manager = Manager::new();
      manager.acquire_lease(|| false, || unreachable!(), || Ok(())).unwrap();
      manager.release_lease_from_finalizer(|| {
        Err(napi::Error::from_reason("finalizer shutdown failed"))
      });

      let recovery_error = manager
        .acquire_manual(
          || Err(napi::Error::from_reason("recovery shutdown failed")),
          || unreachable!(),
        )
        .unwrap_err();
      assert_eq!(recovery_error.reason, "recovery shutdown failed");
      assert_eq!(manager.owner_counts(), (1, 0));
      assert!(manager.has_abandoned_lease_owner());

      manager.acquire_manual(|| Ok(()), || Ok(())).unwrap();
      assert_eq!(manager.owner_counts(), (0, 1));
      assert!(!manager.has_abandoned_lease_owner());
      manager.release_manual(|| Ok(())).unwrap();
    }

    #[test]
    fn panicking_start_and_explicit_shutdown_restore_retryable_state() {
      let manager = Manager::new();
      let start_panic = catch_unwind(AssertUnwindSafe(|| {
        manager
          .acquire_lease(
            || false,
            || unreachable!(),
            || -> napi::Result<()> { panic!("intentional start panic") },
          )
          .unwrap();
      }));
      assert!(start_panic.is_err());
      assert_eq!(manager.owner_count(), 0);

      manager.acquire_lease(|| false, || unreachable!(), || Ok(())).unwrap();
      let shutdown_panic = catch_unwind(AssertUnwindSafe(|| {
        manager
          .release_lease(|| -> napi::Result<()> { panic!("intentional shutdown panic") })
          .unwrap();
      }));
      assert!(shutdown_panic.is_err());
      assert_eq!(manager.owner_count(), 1);

      manager.release_lease(|| Ok(())).unwrap();
      assert_eq!(manager.owner_count(), 0);
    }

    #[test]
    fn panicking_finalizer_is_contained_and_recoverable() {
      let manager = Manager::new();
      manager.acquire_lease(|| false, || unreachable!(), || Ok(())).unwrap();

      let finalizer = catch_unwind(AssertUnwindSafe(|| {
        manager.release_lease_from_finalizer(|| -> napi::Result<()> {
          panic!("intentional finalizer shutdown panic");
        });
      }));
      finalizer.expect("finalizer shutdown panic must remain contained");
      assert_eq!(manager.owner_counts(), (1, 0));
      assert!(manager.has_abandoned_lease_owner());

      manager.acquire_lease(|| false, || Ok(()), || Ok(())).unwrap();
      assert_eq!(manager.owner_counts(), (1, 0));
      assert!(!manager.has_abandoned_lease_owner());
      manager.release_lease(|| Ok(())).unwrap();
    }

    #[test]
    fn concurrent_release_never_underflows() {
      let manager = Arc::new(Manager::new());
      manager.acquire_lease(|| false, || unreachable!(), || Ok(())).unwrap();
      for _ in 1..64 {
        manager.acquire_lease(|| false, || unreachable!(), || unreachable!()).unwrap();
      }
      let shutdown_calls = Arc::new(AtomicUsize::new(0));
      let threads = (0..128)
        .map(|_| {
          let manager = Arc::clone(&manager);
          let shutdown_calls = Arc::clone(&shutdown_calls);
          std::thread::spawn(move || {
            manager
              .release_lease(|| {
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

      assert_eq!(manager.owner_count(), 0);
      assert_eq!(shutdown_calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn manual_shutdown_cannot_release_a_lease_owner() {
      let manager = Manager::new();
      manager.acquire_lease(|| false, || unreachable!(), || Ok(())).unwrap();

      manager.release_manual(|| unreachable!()).unwrap();
      assert_eq!(manager.owner_counts(), (1, 0));

      manager.acquire_manual(|| unreachable!(), || unreachable!()).unwrap();
      assert_eq!(manager.owner_counts(), (1, 1));
      manager.release_manual(|| unreachable!()).unwrap();
      assert_eq!(manager.owner_counts(), (1, 0));
      manager.release_lease(|| Ok(())).unwrap();
      assert_eq!(manager.owner_count(), 0);
    }
  }
}

#[cfg(all(target_family = "wasm", tokio_unstable))]
mod async_runtime_acquisition {
  use std::{
    collections::HashMap,
    sync::{
      Arc, LazyLock, Mutex, Weak,
      atomic::{AtomicBool, Ordering},
    },
  };

  use napi::bindgen_prelude::{
    TokioRuntimeRetirementWaiter, tokio_runtime_retirement_waiter, try_start_async_runtime,
  };

  use super::ASYNC_RUNTIME_LEASES;

  static ENVIRONMENTS: LazyLock<Mutex<HashMap<usize, Weak<EnvironmentCancellation>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

  pub struct AcquisitionCancellation {
    cancelled: AtomicBool,
    waiter: Mutex<Option<TokioRuntimeRetirementWaiter>>,
  }

  impl AcquisitionCancellation {
    fn new() -> Self {
      Self { cancelled: AtomicBool::new(false), waiter: Mutex::new(None) }
    }

    pub fn is_cancelled(&self) -> bool {
      self.cancelled.load(Ordering::Acquire)
    }

    fn cancel(&self) {
      self.cancelled.store(true, Ordering::Release);
      if let Some(waiter) =
        self.waiter.lock().unwrap_or_else(std::sync::PoisonError::into_inner).as_ref()
      {
        waiter.cancel();
      }
      ASYNC_RUNTIME_LEASES.notify_waiters();
    }

    fn wait_for_retirement(&self) -> napi::Result<()> {
      loop {
        if self.is_cancelled() {
          return Err(napi::Error::new(
            napi::Status::Cancelled,
            "Async runtime acquisition was cancelled",
          ));
        }
        let waiter = tokio_runtime_retirement_waiter();
        {
          let mut slot = self.waiter.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
          if self.is_cancelled() {
            waiter.cancel();
          }
          *slot = Some(waiter.clone());
        }
        let wait_result = waiter.wait();
        self.waiter.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take();
        wait_result?;
        if self.is_cancelled() {
          return Err(napi::Error::new(
            napi::Status::Cancelled,
            "Async runtime acquisition was cancelled",
          ));
        }
        match try_start_async_runtime() {
          Ok(()) => return Ok(()),
          Err(error) if error.status == napi::Status::WouldDeadlock => {}
          Err(error) => return Err(error),
        }
      }
    }
  }

  struct EnvironmentCancellation {
    cancelled: AtomicBool,
    acquisitions: Mutex<Vec<Weak<AcquisitionCancellation>>>,
  }

  impl EnvironmentCancellation {
    fn new() -> Self {
      Self { cancelled: AtomicBool::new(false), acquisitions: Mutex::new(Vec::new()) }
    }

    fn register(&self) -> Arc<AcquisitionCancellation> {
      let acquisition = Arc::new(AcquisitionCancellation::new());
      let mut acquisitions =
        self.acquisitions.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
      acquisitions.retain(|entry| entry.strong_count() > 0);
      acquisitions.push(Arc::downgrade(&acquisition));
      if self.cancelled.load(Ordering::Acquire) {
        acquisition.cancel();
      }
      acquisition
    }

    fn cancel(&self) {
      self.cancelled.store(true, Ordering::Release);
      let mut acquisitions =
        self.acquisitions.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
      acquisitions.retain(|entry| {
        let Some(acquisition) = entry.upgrade() else {
          return false;
        };
        acquisition.cancel();
        true
      });
    }
  }

  pub fn register(env: &napi::Env) -> napi::Result<Arc<AcquisitionCancellation>> {
    let env_id = env.raw() as usize;
    let mut environments = ENVIRONMENTS.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(environment) = environments.get(&env_id).and_then(Weak::upgrade) {
      return Ok(environment.register());
    }

    let environment = Arc::new(EnvironmentCancellation::new());
    environments.insert(env_id, Arc::downgrade(&environment));
    let cleanup_environment = Arc::clone(&environment);
    if let Err(error) =
      env.add_env_cleanup_hook((env_id, cleanup_environment), |(env_id, environment)| {
        environment.cancel();
        let mut environments =
          ENVIRONMENTS.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if environments
          .get(&env_id)
          .and_then(Weak::upgrade)
          .is_some_and(|current| Arc::ptr_eq(&current, &environment))
        {
          environments.remove(&env_id);
        }
      })
    {
      environments.remove(&env_id);
      return Err(error);
    }
    drop(environments);
    Ok(environment.register())
  }

  pub fn start(cancellation: &AcquisitionCancellation) -> napi::Result<()> {
    cancellation.wait_for_retirement()
  }
}

#[cfg(all(target_family = "wasm", tokio_unstable))]
// See internal-docs/async-runtime/implementation.md.
static ASYNC_RUNTIME_LEASES: async_runtime_lease::Manager = async_runtime_lease::Manager::new();

pub struct AsyncRuntimeLeaseOwner {
  owned: bool,
}

impl AsyncRuntimeLeaseOwner {
  #[cfg(not(all(target_family = "wasm", tokio_unstable)))]
  const fn noop() -> Self {
    Self { owned: false }
  }

  #[cfg(all(target_family = "wasm", tokio_unstable))]
  const fn acquired() -> Self {
    Self { owned: true }
  }

  #[cfg_attr(
    not(all(target_family = "wasm", tokio_unstable)),
    expect(clippy::unnecessary_wraps, reason = "matches the fallible threaded-WASI lease")
  )]
  fn release(&mut self) -> napi::Result<()> {
    if !self.owned {
      return Ok(());
    }
    #[cfg(all(target_family = "wasm", tokio_unstable))]
    ASYNC_RUNTIME_LEASES.release_lease(napi::bindgen_prelude::try_shutdown_async_runtime)?;
    self.owned = false;
    Ok(())
  }
}

impl Drop for AsyncRuntimeLeaseOwner {
  fn drop(&mut self) {
    if !self.owned {
      return;
    }
    #[cfg(all(target_family = "wasm", tokio_unstable))]
    ASYNC_RUNTIME_LEASES
      .release_lease_from_finalizer(napi::bindgen_prelude::try_shutdown_async_runtime);
    self.owned = false;
  }
}

#[napi]
pub struct BindingAsyncRuntimeLease {
  owner: AsyncRuntimeLeaseOwner,
}

#[napi]
impl BindingAsyncRuntimeLease {
  #[napi]
  pub fn release(&mut self) -> napi::Result<()> {
    self.owner.release()
  }
}

pub struct AcquireAsyncRuntimeTask {
  #[cfg(all(target_family = "wasm", tokio_unstable))]
  cancellation: std::sync::Arc<async_runtime_acquisition::AcquisitionCancellation>,
}

#[napi]
impl napi::Task for AcquireAsyncRuntimeTask {
  type Output = AsyncRuntimeLeaseOwner;
  type JsValue = BindingAsyncRuntimeLease;

  fn compute(&mut self) -> napi::Result<Self::Output> {
    #[cfg(all(target_family = "wasm", tokio_unstable))]
    {
      ASYNC_RUNTIME_LEASES.acquire_lease(
        || self.cancellation.is_cancelled(),
        napi::bindgen_prelude::try_shutdown_async_runtime,
        || async_runtime_acquisition::start(&self.cancellation),
      )?;
      return Ok(AsyncRuntimeLeaseOwner::acquired());
    }
    #[cfg(not(all(target_family = "wasm", tokio_unstable)))]
    Ok(AsyncRuntimeLeaseOwner::noop())
  }

  fn resolve(&mut self, _env: napi::Env, owner: Self::Output) -> napi::Result<Self::JsValue> {
    Ok(BindingAsyncRuntimeLease { owner })
  }
}

#[napi]
#[cfg_attr(
  not(all(target_family = "wasm", tokio_unstable)),
  allow(clippy::unnecessary_wraps, reason = "matches the fallible threaded-WASI export")
)]
pub fn acquire_async_runtime(
  env: &napi::Env,
) -> napi::Result<napi::bindgen_prelude::AsyncTask<AcquireAsyncRuntimeTask>> {
  #[cfg(all(target_family = "wasm", tokio_unstable))]
  {
    return Ok(napi::bindgen_prelude::AsyncTask::new(AcquireAsyncRuntimeTask {
      cancellation: async_runtime_acquisition::register(env)?,
    }));
  }
  #[cfg(not(all(target_family = "wasm", tokio_unstable)))]
  {
    let _ = env;
    Ok(napi::bindgen_prelude::AsyncTask::new(AcquireAsyncRuntimeTask {}))
  }
}

#[cfg(not(all(target_family = "wasm", tokio_unstable)))]
#[expect(clippy::unnecessary_wraps, reason = "matches the fallible threaded-WASI export")]
fn no_op_async_runtime_transition() -> napi::Result<()> {
  Ok(())
}

#[napi]
/// Shutdown one manually retained async runtime owner.
pub fn shutdown_async_runtime() -> napi::Result<()> {
  #[cfg(all(target_family = "wasm", tokio_unstable))]
  {
    return ASYNC_RUNTIME_LEASES.release_manual(napi::bindgen_prelude::try_shutdown_async_runtime);
  }
  #[cfg(not(all(target_family = "wasm", tokio_unstable)))]
  no_op_async_runtime_transition()
}

#[napi]
/// Start and manually retain one async runtime owner.
pub fn start_async_runtime() -> napi::Result<()> {
  #[cfg(all(target_family = "wasm", tokio_unstable))]
  {
    ASYNC_RUNTIME_LEASES.acquire_manual(
      napi::bindgen_prelude::try_shutdown_async_runtime,
      napi::bindgen_prelude::try_start_async_runtime,
    )?;
    return Ok(());
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
