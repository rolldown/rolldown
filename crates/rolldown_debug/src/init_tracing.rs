use std::str::FromStr;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU32;

use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt;
use tracing_subscriber::prelude::*;

use crate::debug_data_propagate_layer::DebugDataPropagateLayer;
use crate::debug_formatter::DebugFormatter;

static TRACER_ID: AtomicU32 = AtomicU32::new(0);

static IS_INITIALIZED: AtomicBool = AtomicBool::new(false);

static FILTER_FOR_DEVTOOL: &str = "[{action}]=trace";

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
  id: u32,
}

impl DebugTracer {
  #[must_use]
  pub fn init() -> Self {
    let id = TRACER_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

    let tracer = Self { id };
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
