use std::{
  fs::OpenOptions,
  io::Write,
  sync::Arc,
  time::{SystemTime, UNIX_EPOCH},
};

use crate::static_data::{DEFAULT_SESSION_ID, OPENED_FILE_HANDLES, OPENED_FILES_BY_SESSION};
use serde::ser::{SerializeMap, Serializer as _};
use tracing::{Event, Subscriber};
use tracing_serde::AsSerde;
use tracing_subscriber::{
  fmt::{FmtContext, FormatEvent, FormatFields, format::Writer},
  registry::LookupSpan,
};

use crate::debug_data_propagate_layer::SessionId;

pub struct DebugFormatter;

impl<S, N> FormatEvent<S, N> for DebugFormatter
where
  S: Subscriber + for<'lookup> LookupSpan<'lookup>,
  N: for<'writer> FormatFields<'writer> + 'static,
{
  fn format_event(
    &self,
    ctx: &FmtContext<'_, S, N>,
    _writer: Writer<'_>,
    event: &Event<'_>,
  ) -> std::fmt::Result {
    let meta = event.metadata();
    let session_id = if let Some(scope) = ctx.event_scope() {
      let mut spans = scope.from_root();
      loop {
        if let Some(span) = spans.next() {
          if let Some(build_id) = span.extensions().get::<SessionId>() {
            break Some(build_id.clone());
          }
        } else {
          break None;
        }
      }
    } else {
      None
    };

    let session_id = session_id.as_ref().map_or(DEFAULT_SESSION_ID, |s| &s.0);

    std::fs::create_dir_all(format!(".rolldown/{session_id}")).ok();

    let log_filename: Arc<str> = format!(".rolldown/{session_id}/logs.json").into();

    if !OPENED_FILE_HANDLES.contains_key(&log_filename) {
      let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_filename.as_ref())
        .map_err(|_| std::fmt::Error)?;
      // Ensure for each file, we only have one unique file handle to prevent multiple writes.
      OPENED_FILE_HANDLES.insert(Arc::clone(&log_filename), file);
    }

    OPENED_FILES_BY_SESSION
      .entry(session_id.to_string())
      .or_default()
      .insert(Arc::clone(&log_filename));

    let mut file = OPENED_FILE_HANDLES
      .get_mut(&log_filename)
      .unwrap_or_else(|| panic!("{log_filename} not found"));
    let mut file = file.value_mut();

    let mut visit = || {
      let mut serializer = serde_json::Serializer::new(&mut file);
      let mut serializer = serializer.serialize_map(None)?;

      serializer.serialize_entry("timestamp", &current_utc_timestamp_ms())?;
      serializer.serialize_entry("level", &meta.level().as_serde())?;
      serializer.serialize_entry("session_id", session_id)?;

      let mut visitor = tracing_serde::SerdeMapVisitor::new(serializer);
      event.record(&mut visitor);

      serializer = visitor.take_serializer()?;

      serializer.end()
    };

    visit().map_err(|_| std::fmt::Error)?;
    writeln!(file).map_err(|_| std::fmt::Error)?;
    file.flush().map_err(|_| std::fmt::Error)?;
    Ok(())
  }
}

fn current_utc_timestamp_ms() -> u128 {
  SystemTime::now().duration_since(UNIX_EPOCH).expect("Time went backwards").as_millis()
}
