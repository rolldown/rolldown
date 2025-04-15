use std::{
  fs::OpenOptions,
  io::Write,
  time::{SystemTime, UNIX_EPOCH},
};

use dashmap::DashMap;
use serde::ser::{SerializeMap, Serializer as _};
use tracing::{Event, Subscriber};
use tracing_serde::AsSerde;
use tracing_subscriber::{
  fmt::{FmtContext, FormatEvent, FormatFields, format::Writer},
  registry::LookupSpan,
};

use crate::build_id_propagate_layer::BuildId;

static OPEN_FILES: std::sync::LazyLock<DashMap<String, std::fs::File>> =
  std::sync::LazyLock::new(DashMap::new);

pub struct DevtoolFormatter;

impl<S, N> FormatEvent<S, N> for DevtoolFormatter
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
    let build_id = if let Some(scope) = ctx.event_scope() {
      let mut spans = scope.from_root();
      loop {
        if let Some(span) = spans.next() {
          if let Some(build_id) = span.extensions().get::<BuildId>() {
            break Some(build_id.clone());
          }
        } else {
          break None;
        }
      }
    } else {
      None
    };

    if let Some(build_id) = &build_id {
      std::fs::create_dir_all(format!(".rolldown/{}", build_id.0)).ok();
    } else {
      std::fs::create_dir_all(".rolldown/default").ok();
    }
    let log_filename =
      build_id.as_ref().map_or(".rolldown/default/log.json".to_string(), |build_id| {
        format!(".rolldown/{}/log.json", build_id.0)
      });

    // TODO(hyf0): While this prevents memory leaks, we should come up with a better solution.
    if OPEN_FILES.len() > 10 {
      let oldest_file = OPEN_FILES.iter().next();
      if let Some(oldest_file) = oldest_file {
        // If we have more than 10 open files, we need to close the oldest one.
        OPEN_FILES.remove(oldest_file.key());
      }
    }

    if !OPEN_FILES.contains_key(&log_filename) {
      let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_filename)
        .map_err(|_| std::fmt::Error)?;
      // Ensure for each file, we only have one unique file handle to prevent multiple writes.
      OPEN_FILES.insert(log_filename.clone(), file);
    }

    let mut file =
      OPEN_FILES.get_mut(&log_filename).unwrap_or_else(|| panic!("{log_filename} not found"));
    let mut file = file.value_mut();

    let visit = || {
      let mut serializer = serde_json::Serializer::new(&mut file);
      let mut serializer = serializer.serialize_map(None)?;

      serializer.serialize_entry("timestamp", &current_utc_timestamp_ms())?;
      serializer.serialize_entry("level", &meta.level().as_serde())?;
      if let Some(build_id) = build_id {
        serializer.serialize_entry("buildId", &build_id.0)?;
      }

      let flatten_event = false;

      if flatten_event {
        let mut visitor = tracing_serde::SerdeMapVisitor::new(serializer);
        event.record(&mut visitor);

        serializer = visitor.take_serializer()?;
      } else {
        use tracing_serde::fields::AsMap;
        serializer.serialize_entry("fields", &event.field_map())?;
      }

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
