use std::{
  future::Future,
  pin::Pin,
  sync::{Arc, atomic::AtomicBool},
};

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
}
