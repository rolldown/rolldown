/// Some guidelines for tracing:
/// - By default, only allow tracing events from crates of this repo.
/// - Using `LOG_LAYER=chrome` to collect tracing events into a json file.
///   - This only works on using `@rolldown/node`. If you are running rolldown in rust, this doesn't works.
/// - Using `RUST_LOG=TRACE` to enable tracing or other values for more specific tracing.
///   - See  https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html#example-syntax for more syntax details.
///   - https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html#directives
use std::sync::atomic::AtomicBool;

use tracing_chrome::FlushGuard;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::{fmt, prelude::*};

static IS_INITIALIZED: AtomicBool = AtomicBool::new(false);

pub fn try_init_tracing() {
  if std::env::var("RUST_LOG").is_err() {
    // tracing will slow down the bundling process, so we only enable it when `LOG` is set.
    return;
  }
  if IS_INITIALIZED.swap(true, std::sync::atomic::Ordering::SeqCst) {
    return;
  }

  tracing_subscriber::registry()
    .with(EnvFilter::from_default_env())
    .with(fmt::layer().pretty().without_time())
    .init();
  tracing::trace!("Tracing is initialized.");
}

pub fn try_init_tracing_with_chrome_layer() -> Option<FlushGuard> {
  use tracing_chrome::ChromeLayerBuilder;
  use tracing_subscriber::prelude::*;
  if std::env::var("RUST_LOG").is_err() {
    // tracing will slow down the bundling process, so we only enable it when `LOG` is set.
    return None;
  }
  if IS_INITIALIZED.swap(true, std::sync::atomic::Ordering::SeqCst) {
    None
  } else {
    let (chrome_layer, guard) = ChromeLayerBuilder::new().build();
    tracing_subscriber::registry().with(EnvFilter::from_default_env()).with(chrome_layer).init();
    Some(guard)
  }
}
