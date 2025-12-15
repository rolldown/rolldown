use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use tracing_subscriber::{filter::FilterFn, fmt, prelude::*};

use crate::devtools_formatter::DevtoolsFormatter;
use crate::devtools_layer::DevtoolsLayer;
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
