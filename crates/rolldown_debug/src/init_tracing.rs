use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt;
use tracing_subscriber::prelude::*;

use crate::debug_data_propagate_layer::DebugDataPropagateLayer;
use crate::debug_formatter::DebugFormatter;
use crate::static_data::OPENED_FILE_HANDLES;
use crate::static_data::OPENED_FILES_BY_SESSION;

static IS_INITIALIZED: AtomicBool = AtomicBool::new(false);

static FILTER_FOR_DEVTOOL: &str = "[{meta}]=trace";

pub fn init_devtool_tracing() {
  if IS_INITIALIZED.swap(true, std::sync::atomic::Ordering::SeqCst) {
    return;
  }

  let env_filter = EnvFilter::from_str(FILTER_FOR_DEVTOOL).unwrap();
  tracing_subscriber::registry()
    .with(env_filter)
    .with(DebugDataPropagateLayer)
    .with(fmt::layer().event_format(DebugFormatter))
    .init();
}

#[allow(dead_code)]
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

    let env_filter = EnvFilter::from_str(FILTER_FOR_DEVTOOL).unwrap();
    tracing_subscriber::registry()
      .with(env_filter)
      .with(DebugDataPropagateLayer)
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
  }
}
