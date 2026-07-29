use std::{
  any::Any,
  error::Error,
  fmt as std_fmt,
  panic::{AssertUnwindSafe, catch_unwind},
  path::Path,
  sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
  },
};

use tracing::{Dispatch, Metadata, Subscriber, subscriber::Interest};
use tracing_subscriber::{
  fmt,
  layer::{Context, Filter},
  prelude::*,
};

use crate::devtools_formatter::DevtoolsFormatter;
use crate::devtools_layer::DevtoolsLayer;
use crate::writer::{self, DevtoolsSessionKey, LogCommand};

static ACTIVE_TRACERS: AtomicUsize = AtomicUsize::new(0);
static TRACING_SUBSCRIBER_STATE: Mutex<TracingSubscriberState> =
  Mutex::new(TracingSubscriberState::Uninitialized);

enum TracingSubscriberState {
  Uninitialized,
  Initialized { capabilities: TracingSubscriberCapabilities, dispatch: Dispatch },
  Failed(DevtoolsTracingInitError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TracingSubscriberCapabilities {
  normal_logging: bool,
}

impl TracingSubscriberCapabilities {
  pub const DEVTOOLS: Self = Self { normal_logging: false };
  pub const DEVTOOLS_AND_LOGGING: Self = Self { normal_logging: true };

  fn supports(self, requested: Self) -> bool {
    !requested.normal_logging || self.normal_logging
  }
}

#[derive(Clone, Debug)]
pub struct DevtoolsTracingInitError {
  message: Arc<str>,
}

impl DevtoolsTracingInitError {
  fn new(message: impl Into<Arc<str>>) -> Self {
    Self { message: message.into() }
  }

  fn unsupported_capabilities(
    installed: TracingSubscriberCapabilities,
    requested: TracingSubscriberCapabilities,
  ) -> Self {
    debug_assert!(!installed.supports(requested));
    Self::new(
      "the Rolldown tracing subscriber was initialized for devtools output only and cannot add \
       normal `RD_LOG` logging after global installation; set `RD_LOG` before creating the first \
       devtools-enabled bundler",
    )
  }
}

impl std_fmt::Display for DevtoolsTracingInitError {
  fn fmt(&self, f: &mut std_fmt::Formatter<'_>) -> std_fmt::Result {
    f.write_str(&self.message)
  }
}

impl Error for DevtoolsTracingInitError {}

pub fn ensure_tracing_subscriber(
  capabilities: TracingSubscriberCapabilities,
  initializer: impl FnOnce() -> Dispatch,
) -> Result<(), DevtoolsTracingInitError> {
  let mut state =
    TRACING_SUBSCRIBER_STATE.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
  match &*state {
    TracingSubscriberState::Initialized { capabilities: installed, .. }
      if installed.supports(capabilities) =>
    {
      return Ok(());
    }
    TracingSubscriberState::Initialized { capabilities: installed, .. } => {
      return Err(DevtoolsTracingInitError::unsupported_capabilities(*installed, capabilities));
    }
    TracingSubscriberState::Failed(error) => return Err(error.clone()),
    TracingSubscriberState::Uninitialized => {}
  }

  let result = match catch_unwind(AssertUnwindSafe(|| {
    let dispatch = initializer();
    tracing::dispatcher::set_global_default(dispatch.clone()).map(|()| dispatch)
  })) {
    Ok(Ok(dispatch)) => Ok(dispatch),
    Ok(Err(error)) => Err(DevtoolsTracingInitError::new(format!(
      "failed to initialize the Rolldown tracing subscriber: {error}"
    ))),
    Err(payload) => {
      let message = panic_payload_message(&*payload);
      discard_panic_payload(payload);
      Err(DevtoolsTracingInitError::new(format!(
        "Rolldown tracing subscriber initialization panicked: {message}"
      )))
    }
  };
  *state = match &result {
    Ok(dispatch) => {
      TracingSubscriberState::Initialized { capabilities, dispatch: dispatch.clone() }
    }
    Err(error) => TracingSubscriberState::Failed(error.clone()),
  };
  result.map(|_| ())
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DevtoolsFilter;

impl<S> Filter<S> for DevtoolsFilter
where
  S: Subscriber,
{
  fn enabled(&self, metadata: &Metadata<'_>, _context: &Context<'_, S>) -> bool {
    if is_devtools_action_event(metadata) {
      // Action callsites are globally disabled while no tracer exists. When a
      // normal tracing layer creates mixed per-layer interest, avoid an atomic
      // load on every action event in the active steady state.
      return true;
    }
    ACTIVE_TRACERS.load(Ordering::Acquire) > 0 && metadata.is_span()
  }

  fn callsite_enabled(&self, metadata: &'static Metadata<'static>) -> Interest {
    devtools_callsite_interest(metadata)
  }
}

fn devtools_callsite_interest(metadata: &'static Metadata<'static>) -> Interest {
  if ACTIVE_TRACERS.load(Ordering::Acquire) > 0 && is_devtools_metadata(metadata) {
    Interest::always()
  } else {
    Interest::never()
  }
}

fn is_devtools_metadata(metadata: &Metadata<'_>) -> bool {
  metadata.is_span() || is_devtools_action_event(metadata)
}

fn is_devtools_action_event(metadata: &Metadata<'_>) -> bool {
  metadata.is_event() && metadata.fields().field("devtoolsAction").is_some()
}

fn acquire_tracer_interest() {
  if ACTIVE_TRACERS.fetch_add(1, Ordering::AcqRel) == 0 {
    rebuild_interest_cache();
  }
}

fn release_tracer_interest() {
  let previous = ACTIVE_TRACERS.fetch_sub(1, Ordering::AcqRel);
  debug_assert!(previous > 0, "devtools tracer count underflow");
  if previous == 1 {
    rebuild_interest_cache();
  }
}

fn rebuild_interest_cache() {
  let dispatch = {
    let state = TRACING_SUBSCRIBER_STATE.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    match &*state {
      TracingSubscriberState::Initialized { dispatch, .. } => dispatch.clone(),
      TracingSubscriberState::Uninitialized | TracingSubscriberState::Failed(_) => return,
    }
  };
  tracing::dispatcher::with_default(&dispatch, tracing::callsite::rebuild_interest_cache);
}

#[derive(Debug, Clone)]
pub struct DebugTracer {
  lease: Arc<DebugTracerLease>,
}

#[derive(Debug)]
struct DebugTracerLease {
  session: DevtoolsSessionKey,
}

impl DebugTracer {
  pub fn init(session_id: Arc<str>, cwd: &Path) -> Result<Self, DevtoolsTracingInitError> {
    ensure_tracing_subscriber(TracingSubscriberCapabilities::DEVTOOLS, || {
      Dispatch::new(
        tracing_subscriber::registry()
          .with(DevtoolsLayer.with_filter(DevtoolsFilter))
          .with(fmt::layer().event_format(DevtoolsFormatter).with_filter(DevtoolsFilter)),
      )
    })?;

    let session = DevtoolsSessionKey::new(session_id, cwd);
    writer::register_session_owner(session.clone());
    acquire_tracer_interest();
    let tracer = Self { lease: Arc::new(DebugTracerLease { session }) };
    Ok(tracer)
  }

  pub fn session_key(&self) -> &DevtoolsSessionKey {
    &self.lease.session
  }
}

impl Drop for DebugTracerLease {
  fn drop(&mut self) {
    // Best-effort cleanup path. Callers that need "file readable after this
    // call" semantics should use `flush_session(...)` instead, which returns a
    // receiver with the result after the writer backend drains this session.
    // This fallback must not replace that structured result with a panic if the
    // process-global writer failed to initialize or is poisoned.
    writer::send_best_effort(LogCommand::CloseSession { session: self.session.clone(), ack: None });
    release_tracer_interest();
  }
}

fn panic_payload_message(payload: &(dyn Any + Send)) -> String {
  if let Some(message) = payload.downcast_ref::<String>() {
    message.clone()
  } else if let Some(message) = payload.downcast_ref::<&str>() {
    (*message).to_string()
  } else {
    "non-string panic payload".to_string()
  }
}

fn discard_panic_payload(payload: Box<dyn Any + Send>) {
  if let Err(nested_payload) = catch_unwind(AssertUnwindSafe(|| drop(payload))) {
    std::mem::forget(nested_payload);
  }
}

#[derive(Debug, Clone)]

pub struct Session {
  pub id: Arc<str>,
  pub span: tracing::Span,
}

impl Session {
  pub fn new(id: Arc<str>, span: tracing::Span) -> Self {
    Self { id, span }
  }

  pub fn dummy() -> Self {
    let session_id = Arc::from("unknown_session");
    Self { id: session_id, span: tracing::Span::none() }
  }
}
