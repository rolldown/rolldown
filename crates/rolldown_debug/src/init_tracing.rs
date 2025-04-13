use std::str::FromStr;
use std::sync::atomic::AtomicBool;

use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt;
use tracing_subscriber::prelude::*;

use crate::build_id_propagate_layer::BuildIdPropagateLayer;
use crate::debug_formatter::DevtoolFormatter;

static IS_INITIALIZED: AtomicBool = AtomicBool::new(false);

static FILTER_FOR_DEVTOOL: &str = "[{action}]=trace";

pub fn init_devtool_tracing() {
  if IS_INITIALIZED.swap(true, std::sync::atomic::Ordering::SeqCst) {
    return;
  }

  let env_filter = EnvFilter::from_str(FILTER_FOR_DEVTOOL).unwrap();
  tracing_subscriber::registry()
    .with(env_filter)
    .with(BuildIdPropagateLayer)
    .with(fmt::layer().event_format(DevtoolFormatter))
    .init();
}
