use std::{
  any::Any,
  fmt,
  future::Future,
  panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
  pin::Pin,
  sync::{Arc, Mutex, PoisonError},
  task::{Context, Poll},
};

use arcstr::ArcStr;
use async_lock::Mutex as TokioMutex;
use futures::{FutureExt, future::Shared};
use rolldown_common::HmrStampTable;
use rolldown_dev_common::types::{DevCallbackError, DevCallbackResult};
use rolldown_error::{BatchedBuildDiagnostic, BuildResult};
use rustc_hash::FxHashMap;

use crate::{
  NormalizedDevOptions, SharedClients, type_aliases::CoordinatorSender,
  types::pending_payload::PendingPayload,
};

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

#[derive(Debug)]
pub struct RetainedDevCallbackErrors(Vec<DevCallbackError>);

impl RetainedDevCallbackErrors {
  pub fn into_error(errors: Vec<DevCallbackError>) -> DevCallbackError {
    Arc::new(Self(errors))
  }
}

impl std::fmt::Display for RetainedDevCallbackErrors {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    self.0.iter().map(std::string::ToString::to_string).collect::<Vec<_>>().join("\n").fmt(f)
  }
}

impl std::error::Error for RetainedDevCallbackErrors {
  fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
    self.0.first().map(|error| error.as_ref() as &dyn std::error::Error)
  }
}

pub fn dev_callback_result_to_build_result(result: DevCallbackResult) -> BuildResult<()> {
  result.map_err(|error| BatchedBuildDiagnostic::new(dev_callback_error_to_diagnostics(error)))
}

fn dev_callback_error_to_diagnostics(
  error: DevCallbackError,
) -> Vec<rolldown_error::BuildDiagnostic> {
  if let Some(errors) = error.as_ref().downcast_ref::<RetainedDevCallbackErrors>() {
    return errors
      .0
      .iter()
      .flat_map(|error| dev_callback_error_to_diagnostics(Arc::clone(error)))
      .collect();
  }

  BatchedBuildDiagnostic::from(anyhow::Error::new(RetainedDevCallbackError(error))).into_vec()
}

/// Keep at most this many rendered-but-undelivered payloads per client. A dropped
/// entry just degrades to the existing delivery-failure reload path: its modules
/// stay stale in the ship map, so a later push re-ships or full-reloads them.
const MAX_PENDING_PAYLOADS_PER_CLIENT: usize = 8;

pub struct DevContext {
  pub options: NormalizedDevOptions,
  pub coordinator_tx: CoordinatorSender,
  pub clients: SharedClients,
  /// Dev-engine-wide rebuild-stamp ship map of the versioned delivery protocol.
  pub stamp_table: Arc<TokioMutex<HmrStampTable>>,
  /// Rendered-but-not-yet-delivered payloads, keyed by output filename. The
  /// delivery notification consumes an entry when the serving middleware sees
  /// the response for that filename complete.
  pub pending_payloads: Arc<TokioMutex<FxHashMap<String, PendingPayload>>>,
  /// Boot-evaluated map of the latest written bundle output: module stable id →
  /// render stamp of the copy the entry chunk evaluates at top level (computed
  /// statically — see `Bundler::compute_top_level_evaluated_modules`). Swapped whole
  /// after every successful rebuild; `register_client` freezes the then-current
  /// `Arc` into the new session, since a hello can only come from the runtime
  /// inside a served entry chunk.
  pub top_level_evaluated: TokioMutex<Arc<FxHashMap<ArcStr, u32>>>,
}

impl DevContext {
  /// Record a rendered payload as pending so the delivery notification can
  /// max-merge its stamps into that client's `shipped[C]` once the serving
  /// middleware observes the response complete.
  ///
  /// Bounds per-client growth: past `MAX_PENDING_PAYLOADS_PER_CLIENT` entries
  /// the oldest ones are dropped — see the constant's doc for why that is safe.
  pub async fn insert_pending_payload(&self, filename: String, payload: PendingPayload) {
    let client_id = payload.client_id.clone();
    let mut pending_payloads = self.pending_payloads.lock().await;
    pending_payloads.insert(filename, payload);

    // Count first: the common case is far below the bound, so don't build the
    // eviction list (filename clones + id parses) until it is actually needed.
    let client_count =
      pending_payloads.values().filter(|payload| payload.client_id == client_id).count();
    if client_count > MAX_PENDING_PAYLOADS_PER_CLIENT {
      let mut client_entries = pending_payloads
        .iter()
        .filter(|(_, payload)| payload.client_id == client_id)
        .map(|(filename, _)| (patch_id_of(filename), filename.clone()))
        .collect::<Vec<_>>();
      client_entries.sort_unstable();
      for (_, filename) in &client_entries[..client_entries.len() - MAX_PENDING_PAYLOADS_PER_CLIENT]
      {
        pending_payloads.remove(filename);
      }
    }
  }
}

/// The numeric id embedded in a payload filename (`hmr_patch_{id}.js` /
/// `lazy_compile_{id}.js`). Both formats draw from the engine's single patch-id
/// counter, so the id orders pending entries by age across the two kinds.
fn patch_id_of(filename: &str) -> u32 {
  filename
    .rsplit('_')
    .next()
    .and_then(|rest| rest.strip_suffix(".js"))
    .and_then(|id| id.parse().ok())
    .unwrap_or(0)
}
