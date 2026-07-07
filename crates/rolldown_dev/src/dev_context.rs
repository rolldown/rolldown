use std::{
  any::Any,
  fmt,
  future::Future,
  panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
  pin::Pin,
  sync::{Arc, Mutex, PoisonError},
  task::{Context, Poll},
};

use futures::{FutureExt, future::Shared};
use rolldown_dev_common::types::{DevCallbackError, DevCallbackResult};
use rolldown_error::{BatchedBuildDiagnostic, BuildResult};

use crate::{NormalizedDevOptions, SharedClients, type_aliases::CoordinatorSender};

pub type SharedDevContext = Arc<DevContext>;

pub type PinBoxSendStaticFuture<T = ()> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

#[derive(Clone)]
enum BundlingTaskOutcome {
  Completed(DevCallbackResult),
  Panicked(BundlingTaskPanic),
}

#[derive(Clone)]
enum BundlingTaskPanic {
  StaticStr(&'static str),
  String(Arc<str>),
  Opaque(Arc<OpaquePanicPayload>),
}

struct OpaquePanicPayload {
  payload: Mutex<Option<Box<dyn Any + Send>>>,
}

impl BundlingTaskPanic {
  fn new(payload: Box<dyn Any + Send>) -> Self {
    let payload = match payload.downcast::<String>() {
      Ok(message) => return Self::String(Arc::from(message.as_str())),
      Err(payload) => payload,
    };
    match payload.downcast::<&'static str>() {
      Ok(message) => Self::StaticStr(*message),
      Err(payload) => {
        Self::Opaque(Arc::new(OpaquePanicPayload { payload: Mutex::new(Some(payload)) }))
      }
    }
  }

  fn resume(&self) -> ! {
    match self {
      Self::StaticStr(message) => resume_unwind(Box::new(*message)),
      Self::String(message) => resume_unwind(Box::new(message.to_string())),
      Self::Opaque(payload) => payload.resume(),
    }
  }
}

impl OpaquePanicPayload {
  fn resume(&self) -> ! {
    let payload =
      self.payload.lock().unwrap_or_else(PoisonError::into_inner).take().unwrap_or_else(|| {
        Box::new("non-cloneable bundling task panic payload was already resumed")
      });
    resume_unwind(payload)
  }
}

impl Drop for OpaquePanicPayload {
  fn drop(&mut self) {
    let payload = self.payload.get_mut().unwrap_or_else(PoisonError::into_inner).take();
    if let Some(payload) = payload {
      discard_panic_payload(payload);
    }
  }
}

type SharedBundlingTaskOutcome = Shared<PinBoxSendStaticFuture<BundlingTaskOutcome>>;

/// A cloneable waiter for an ongoing `BundlingTask`.
///
/// Panics are captured as data before entering `Shared`, then replayed by this
/// outer future. This lets `Shared` publish a completed state and wake every
/// waiter before any one waiter resumes the panic. String payloads retain their
/// original type for every waiter; the first observer of an opaque payload
/// receives that payload unchanged.
#[derive(Clone)]
pub struct BundlingFuture {
  inner: SharedBundlingTaskOutcome,
}

impl fmt::Debug for BundlingFuture {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.debug_struct("BundlingFuture").finish_non_exhaustive()
  }
}

impl BundlingFuture {
  pub(crate) fn new<F>(future: F) -> Self
  where
    F: Future<Output = DevCallbackResult> + Send + 'static,
  {
    let future = async move {
      match AssertUnwindSafe(future).catch_unwind().await {
        Ok(result) => BundlingTaskOutcome::Completed(result),
        Err(payload) => BundlingTaskOutcome::Panicked(BundlingTaskPanic::new(payload)),
      }
    };
    Self { inner: (Box::pin(future) as PinBoxSendStaticFuture<BundlingTaskOutcome>).shared() }
  }

  pub(crate) async fn drive(self) {
    let _ = self.inner.await;
  }

  #[cfg(feature = "testing")]
  pub async fn drive_for_testing(self) {
    self.drive().await;
  }

  #[cfg(feature = "testing")]
  pub fn new_for_testing<F>(future: F) -> Self
  where
    F: Future<Output = DevCallbackResult> + Send + 'static,
  {
    Self::new(future)
  }
}

impl Future for BundlingFuture {
  type Output = DevCallbackResult;

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    match Pin::new(&mut self.get_mut().inner).poll(cx) {
      Poll::Pending => Poll::Pending,
      Poll::Ready(BundlingTaskOutcome::Completed(result)) => Poll::Ready(result),
      Poll::Ready(BundlingTaskOutcome::Panicked(payload)) => payload.resume(),
    }
  }
}

fn discard_panic_payload(payload: Box<dyn Any + Send>) {
  if let Err(payload) = catch_unwind(AssertUnwindSafe(|| drop(payload)))
    && let Err(nested_payload) = catch_unwind(AssertUnwindSafe(|| drop(payload)))
  {
    std::mem::forget(nested_payload);
  }
}

#[derive(Debug)]
struct RetainedDevCallbackError(DevCallbackError);

impl std::fmt::Display for RetainedDevCallbackError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    self.0.fmt(f)
  }
}

impl std::error::Error for RetainedDevCallbackError {
  fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
    Some(self.0.as_ref())
  }
}

pub fn dev_callback_result_to_build_result(result: DevCallbackResult) -> BuildResult<()> {
  result.map_err(|error| {
    BatchedBuildDiagnostic::from(anyhow::Error::new(RetainedDevCallbackError(error)))
  })
}

pub struct DevContext {
  pub options: NormalizedDevOptions,
  pub coordinator_tx: CoordinatorSender,
  pub clients: SharedClients,
}
