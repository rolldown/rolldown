/// Some guidelines for tracing:
/// - Using `RD_LOG=trace` to enable tracing or other values for more specific tracing.
///   - See  https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html#example-syntax for more syntax details.
///   - https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html#directives
/// - Using `RD_LOG=trace RD_LOG_OUTPUT=chrome-json` to collect tracing events into a json file.
use std::sync::atomic::AtomicBool;

use tracing_chrome::ChromeLayerBuilder;
use tracing_chrome::FlushGuard;
use tracing_subscriber::fmt;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::prelude::*;
use tracing_subscriber::EnvFilter;

static LOG_ENV_NAME: &str = "RD_LOG";
static LOG_OUTPUT_ENV_NAME: &str = "RD_LOG_OUTPUT";

static IS_INITIALIZED: AtomicBool = AtomicBool::new(false);

pub fn try_init_tracing() -> Option<FlushGuard> {
  if std::env::var(LOG_ENV_NAME).is_err() {
    // tracing will slow down the bundling process, so we only enable it when `LOG` is set.
    return None;
  }
  if IS_INITIALIZED.swap(true, std::sync::atomic::Ordering::SeqCst) {
    return None;
  }

  let output_mode = std::env::var(LOG_OUTPUT_ENV_NAME).unwrap_or_else(|_| "stdout".to_string());

  match output_mode.as_str() {
    "chrome-json" => {
      let (chrome_layer, guard) = ChromeLayerBuilder::new().build();
      tracing_subscriber::registry().with(EnvFilter::from_default_env()).with(chrome_layer).init();
      Some(guard)
    }
    "json" => {
      // We gonna use this feature to implement something like https://github.com/antfu-collective/vite-plugin-inspect
      unimplemented!()
    }
    _ => {
      tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(fmt::layer().pretty().without_time().with_span_events(FmtSpan::EXIT))
        .init();
      tracing::trace!("Tracing is initialized.");
      None
    }
  }
}
