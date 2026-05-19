use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use tracing_subscriber::{filter::FilterFn, fmt, prelude::*};

use crate::devtools_formatter::DevtoolsFormatter;
use crate::devtools_layer::DevtoolsLayer;
use crate::writer::{self, LogCommand};

static IS_INITIALIZED: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone)]
pub struct DebugTracer {
  session_id: Arc<str>,
}

impl DebugTracer {
  #[must_use]
  pub fn init(session_id: Arc<str>) -> Self {
    let tracer = Self { session_id };
    if IS_INITIALIZED.swap(true, std::sync::atomic::Ordering::SeqCst) {
      return tracer;
    }

    let devtools_event_filter = FilterFn::new(|metadata| {
      const ALLOW: bool = true;
      const REJECT: bool = false;
      if metadata.is_event() {
        if metadata.fields().field("devtoolsAction").is_some() { ALLOW } else { REJECT }
      } else {
        // Spans for devtool don't have character data so far.
        ALLOW
      }
    });

    tracing_subscriber::registry()
      .with(DevtoolsLayer.with_filter(devtools_event_filter))
      .with(fmt::layer().event_format(DevtoolsFormatter))
      .init();

    tracer
  }
}

impl Drop for DebugTracer {
  fn drop(&mut self) {
    // Best-effort cleanup path. Callers that need "file readable after this
    // call" semantics should use `flush_session(...)` instead, which returns a
    // receiver signalled after the writer thread drains this session.
    writer::send(LogCommand::CloseSession {
      session_id: self.session_id.as_ref().to_string(),
      ack: None,
    });
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
