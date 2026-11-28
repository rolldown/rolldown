use std::{
  future::Future,
  pin::Pin,
  sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
  },
};

use event_listener::Event;
use futures::future::Shared;

use crate::{NormalizedDevOptions, type_aliases::CoordinatorSender};

pub type SharedDevContext = Arc<DevContext>;

pub type PinBoxSendStaticFuture<T = ()> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

// The future represents an ongoing `BundlingTask`
pub type BundlingFuture = Shared<PinBoxSendStaticFuture<()>>;

pub struct DevContext {
  pub options: NormalizedDevOptions,
  pub coordinator_tx: CoordinatorSender,
  /// Whether the previous bundling task errored. The next HMR compute passes it
  /// as `last_build_errored` to disable the unchanged-output suppression, so a
  /// byte-identical recovery still reaches clients stuck on that error. The next
  /// task also reads it to clear the resolver cache
  /// (`BundlingTask::should_clear_resolver_cache`). Written at the end of every
  /// task; tasks are serialized by the coordinator.
  pub last_task_errored: AtomicBool,
  /// Counts the patch and lazy requests that reached the engine. A task that pushed an update
  /// waits until it stops changing before its rebuild (`BundlingTask::wait_for_pulls`).
  pub pull_requests: AtomicU64,
  is_closed: AtomicBool,
  close_event: Event,
}

impl DevContext {
  pub fn new(options: NormalizedDevOptions, coordinator_tx: CoordinatorSender) -> Self {
    Self {
      options,
      coordinator_tx,
      last_task_errored: AtomicBool::new(false),
      pull_requests: AtomicU64::new(0),
      is_closed: AtomicBool::new(false),
      close_event: Event::new(),
    }
  }

  pub fn is_closed(&self) -> bool {
    self.is_closed.load(Ordering::SeqCst)
  }

  /// Returns `false` when the engine was already closed.
  pub fn mark_closed(&self) -> bool {
    if self.is_closed.swap(true, Ordering::SeqCst) {
      return false;
    }
    self.close_event.notify(usize::MAX);
    true
  }

  /// Ends when the engine is closed.
  pub async fn closed(&self) {
    // `Event` stores no notification, so listen before the check.
    let listener = self.close_event.listen();
    if self.is_closed() {
      return;
    }
    listener.await;
  }
}
