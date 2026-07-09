/// Some guidelines for tracing:
/// - Using `RD_LOG=trace` to enable tracing or other values for more specific tracing.
///   - See  https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html#example-syntax for more syntax details.
///   - https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html#directives
/// - Using `RD_LOG=trace RD_LOG_OUTPUT=chrome-json` to collect tracing events into a json file.
///   - Using `RD_LOG_OUTPUT_STYLE=async` to record traces as a group of asynchronous operations.
///   - Requires building with the `chrome-tracing` feature, which is enabled for profile
///     builds but disabled in release builds to keep the shipped binary smaller.
use std::sync::atomic::AtomicBool;
use std::{any::Any, str::FromStr};

#[cfg(feature = "chrome-tracing")]
use tracing_chrome::ChromeLayerBuilder;
#[cfg(feature = "chrome-tracing")]
use tracing_chrome::TraceStyle;
use tracing_subscriber::filter::filter_fn;
use tracing_subscriber::{
  filter::Targets,
  fmt::{self, format::FmtSpan},
  prelude::*,
};

static LOG_ENV_NAME: &str = "RD_LOG";
static LOG_OUTPUT_ENV_NAME: &str = "RD_LOG_OUTPUT";

static IS_INITIALIZED: AtomicBool = AtomicBool::new(false);

macro_rules! with_devtools_layers {
  ($subscriber:expr) => {
    $subscriber
      .with(rolldown_devtools::DevtoolsLayer.with_filter(rolldown_devtools::DevtoolsFilter))
      .with(
        fmt::layer()
          .event_format(rolldown_devtools::DevtoolsFormatter)
          .with_filter(rolldown_devtools::DevtoolsFilter),
      )
  };
}

pub fn try_init_tracing() -> Option<Box<dyn Any + Send>> {
  let Ok(env_var) = std::env::var(LOG_ENV_NAME) else {
    // tracing will slow down the bundling process, so we only enable it when `LOG` is set.
    return None;
  };
  if IS_INITIALIZED.swap(true, std::sync::atomic::Ordering::SeqCst) {
    return None;
  }

  let output_mode = std::env::var(LOG_OUTPUT_ENV_NAME).unwrap_or_else(|_| "stdout".to_string());
  let targets = match Targets::from_str(&env_var) {
    Ok(targets) => targets,
    Err(error) => {
      report_tracing_init_failure(&format!("invalid `{LOG_ENV_NAME}` filter: {error}"));
      return None;
    }
  };

  // Remove events that have `devtoolsAction` field, as those events are only for devtools.
  let filter_for_removing_devtools_event = filter_fn(|metadata| {
    const ALLOW: bool = true;
    const REJECT: bool = false;
    if metadata.is_event() && metadata.fields().field("devtoolsAction").is_some() {
      return REJECT;
    }
    ALLOW
  });

  match output_mode.as_str() {
    "chrome-json" | "chrome-json-threaded" => {
      #[cfg(feature = "chrome-tracing")]
      {
        let trace_style =
          if output_mode == "chrome-json" { TraceStyle::Async } else { TraceStyle::Threaded };
        let (chrome_layer, guard) =
          ChromeLayerBuilder::new().trace_style(trace_style).include_args(true).build();
        if !initialize_tracing(with_devtools_layers!(
          tracing_subscriber::registry().with(
            chrome_layer.with_filter(filter_for_removing_devtools_event).with_filter(targets),
          )
        )) {
          return None;
        }
        Some(Box::new(guard))
      }
      #[cfg(not(feature = "chrome-tracing"))]
      {
        #![expect(clippy::print_stderr, reason = "Warn before tracing is initialized")]
        eprintln!(
          "`RD_LOG_OUTPUT={output_mode}` requires building with the `chrome-tracing` feature, \
           which is disabled in release builds. Falling back to readable stdout output. \
           Build a profile binary (`pnpm build-binding:profile`) to enable chrome tracing."
        );
        if !initialize_tracing(with_devtools_layers!(
          tracing_subscriber::registry().with(
            fmt::layer()
              .pretty()
              .with_span_events(FmtSpan::NONE)
              .with_level(true)
              .with_target(false)
              .with_filter(filter_for_removing_devtools_event)
              .with_filter(targets),
          )
        )) {
          return None;
        }
        None
      }
    }
    "json" => {
      report_tracing_init_failure(
        "`RD_LOG_OUTPUT=json` is not implemented; falling back to readable output",
      );
      if !initialize_tracing(with_devtools_layers!(
        tracing_subscriber::registry().with(
          fmt::layer()
            .pretty()
            .with_span_events(FmtSpan::NONE)
            .with_level(true)
            .with_target(false)
            .with_filter(filter_for_removing_devtools_event)
            .with_filter(targets),
        )
      )) {
        return None;
      }
      None
    }
    "readable" => {
      if !initialize_tracing(with_devtools_layers!(
        tracing_subscriber::registry().with(
          fmt::layer()
            .pretty()
            .with_span_events(FmtSpan::NONE)
            .with_level(true)
            .with_target(false)
            .with_filter(filter_for_removing_devtools_event)
            .with_filter(targets),
        )
      )) {
        return None;
      }
      tracing::debug!("Tracing initialized");
      None
    }
    _ => {
      if !initialize_tracing(with_devtools_layers!(
        tracing_subscriber::registry().with(
          fmt::layer()
            .pretty()
            .with_span_events(FmtSpan::CLOSE | FmtSpan::ENTER)
            .with_filter(filter_for_removing_devtools_event)
            .with_filter(targets),
        )
      )) {
        return None;
      }
      tracing::debug!("Tracing initialized");
      None
    }
  }
}

fn initialize_tracing(subscriber: impl tracing::Subscriber + Send + Sync + 'static) -> bool {
  match rolldown_devtools::ensure_tracing_subscriber(
    rolldown_devtools::TracingSubscriberCapabilities::DEVTOOLS_AND_LOGGING,
    || tracing::Dispatch::new(subscriber),
  ) {
    Ok(()) => true,
    Err(error) => {
      report_tracing_init_failure(&error.to_string());
      false
    }
  }
}

#[expect(
  clippy::print_stderr,
  reason = "tracing is unavailable for reporting its own init failure"
)]
fn report_tracing_init_failure(message: &str) {
  eprintln!("Rolldown tracing disabled: {message}");
}
