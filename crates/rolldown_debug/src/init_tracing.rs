use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use tracing::Level;
use tracing_subscriber::{filter::FilterFn, fmt, prelude::*};

use crate::debug_data_propagate_layer::DebugDataPropagateLayer;
use crate::debug_formatter::DebugFormatter;
use crate::static_data::EXIST_HASH_BY_SESSION;
use crate::static_data::OPENED_FILE_HANDLES;
use crate::static_data::OPENED_FILES_BY_SESSION;

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

    tracing_subscriber::registry()
      .with(DebugDataPropagateLayer.with_filter(FilterFn::new(|metadata| {
        // Corresponds to `tracing::trace!(meta = ...)` defined by `trace_action`.
        metadata.fields().field("meta").is_some() && *metadata.level() == Level::TRACE
      })))
      .with(fmt::layer().event_format(DebugFormatter))
      .init();

    tracer
  }
}

impl Drop for DebugTracer {
  fn drop(&mut self) {
    if let Some((_session_id, files)) = OPENED_FILES_BY_SESSION.remove(self.session_id.as_ref()) {
      for file in files {
        OPENED_FILE_HANDLES.remove(&file);
      }
    }
    EXIST_HASH_BY_SESSION.remove(self.session_id.as_ref());
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
