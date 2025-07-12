/// Some guidelines for tracing:
/// - Using `RD_LOG=trace` to enable tracing or other values for more specific tracing.
///   - See  https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html#example-syntax for more syntax details.
///   - https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html#directives
/// - Using `RD_LOG=trace RD_LOG_OUTPUT=chrome-json` to collect tracing events into a json file.
///   - Using `RD_LOG_OUTPUT_STYLE=async` to record traces as a group of asynchronous operations.
use std::sync::atomic::AtomicBool;
use std::{any::Any, str::FromStr};

use tracing_chrome::ChromeLayerBuilder;
use tracing_chrome::TraceStyle;
use tracing_subscriber::{
  filter::Targets,
  fmt::{self, format::FmtSpan},
  prelude::*,
};

static LOG_ENV_NAME: &str = "RD_LOG";
static LOG_OUTPUT_ENV_NAME: &str = "RD_LOG_OUTPUT";

static IS_INITIALIZED: AtomicBool = AtomicBool::new(false);

pub fn try_init_tracing() -> Option<Box<dyn Any + Send>> {
  let Ok(env_var) = std::env::var(LOG_ENV_NAME) else {
    // tracing will slow down the bundling process, so we only enable it when `LOG` is set.
    return None;
  };
  if IS_INITIALIZED.swap(true, std::sync::atomic::Ordering::SeqCst) {
    return None;
  }

  let output_mode = std::env::var(LOG_OUTPUT_ENV_NAME).unwrap_or_else(|_| "stdout".to_string());

  match output_mode.as_str() {
    "chrome-json" | "chrome-json-threaded" => {
      let trace_style =
        if output_mode == "chrome-json" { TraceStyle::Async } else { TraceStyle::Threaded };
      let (chrome_layer, guard) =
        ChromeLayerBuilder::new().trace_style(trace_style).include_args(true).build();
      tracing_subscriber::registry()
        .with(Targets::from_str(&env_var).unwrap())
        .with(chrome_layer)
        .init();
      Some(Box::new(guard))
    }
    "json" => {
      // We gonna use this feature to implement something like https://github.com/antfu-collective/vite-plugin-inspect
      // See `crates/rolldown_debug`
      unimplemented!()
    }
    _ => {
      tracing_subscriber::registry()
        .with(Targets::from_str(&env_var).unwrap())
        .with(fmt::layer().pretty().with_span_events(FmtSpan::CLOSE | FmtSpan::ENTER))
        .init();
      tracing::debug!("Tracing initialized");
      None
    }
  }
}
