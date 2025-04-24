use std::{
  fs::OpenOptions,
  io::Write,
  sync::Arc,
  time::{SystemTime, UNIX_EPOCH},
};

use crate::{
  debug_data_propagate_layer::ContextData,
  static_data::{DEFAULT_SESSION_ID, OPENED_FILE_HANDLES, OPENED_FILES_BY_SESSION},
};
use rustc_hash::FxHashMap;
use serde::ser::{SerializeMap, Serializer as _};
use tracing::{Event, Subscriber};
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
    let mut action_meta_extractor = ActionMetaExtractor::default();
    event.record(&mut action_meta_extractor);
    let action_meta = action_meta_extractor.meta;

    if let Some(mut action_meta) = action_meta {
      let mut context_variables = extract_context_variables(&action_meta);
      let mut captured_values = FxHashMap::default();

      if let Some(scope) = ctx.event_scope() {
        for span in scope {
          let span_extensions = span.extensions();
          let Some(context_data) = span_extensions.get::<ContextData>() else {
            continue;
          };
          let found_field_indexes = context_variables
            .iter()
            .enumerate()
            .filter_map(|(idx, name)| {
              if let Some(value) = context_data.get(name.as_str()) {
                captured_values.insert(name.clone(), value.clone());
                Some(idx)
              } else {
                None
              }
            })
            .collect::<Vec<_>>();

          found_field_indexes.iter().rev().for_each(|idx| {
            context_variables.swap_remove(*idx);
          });
        }
      }

      inject_context_data(&mut action_meta, &captured_values);

      // This branch means this event is not only for normal tracing, but also for devtool tracing.
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
        // WARN: Do not use pretty print here, vite-devtool relies on the format of every line is a json object.
        let mut serializer = serde_json::Serializer::new(&mut file);
        let mut serializer = serializer.serialize_map(None)?;

        serializer.serialize_entry("timestamp", &current_utc_timestamp_ms())?;
        serializer.serialize_entry("session_id", session_id)?;
        let serde_json::Value::Object(action_meta) = &action_meta else {
          unreachable!("action_meta should always be an object");
        };

        for (key, value) in action_meta {
          serializer.serialize_entry(key, value)?;
        }

        // TODO(hyf0): we don't care about other fields for now.
        // let mut visitor = tracing_serde::SerdeMapVisitor::new(serializer);
        // event.record(&mut visitor);
        // serializer = visitor.take_serializer()?;

        serializer.end()
      };

      visit().map_err(|_| std::fmt::Error)?;
      writeln!(file).map_err(|_| std::fmt::Error)?;
      file.flush().map_err(|_| std::fmt::Error)?;
      Ok(())
    } else {
      // This branch means this event for normal tracing.
      Ok(())
    }
  }
}

fn current_utc_timestamp_ms() -> u128 {
  SystemTime::now().duration_since(UNIX_EPOCH).expect("Time went backwards").as_millis()
}

#[derive(Default)]
pub struct ActionMetaExtractor {
  pub meta: Option<serde_json::Value>,
}

impl tracing::field::Visit for ActionMetaExtractor {
  fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
    if field.name() == "meta" {
      self.meta = Some(serde_json::from_str(value).unwrap());
    }
  }

  fn record_debug(&mut self, _field: &tracing::field::Field, _value: &dyn std::fmt::Debug) {
    // `EventMetaExtractor` doesn't care about debug values.
  }
}

pub fn extract_context_variables(meta: &serde_json::Value) -> Vec<String> {
  fn visit(meta: &serde_json::Value, context_variables: &mut Vec<String>) {
    if let serde_json::Value::Object(map) = meta {
      for (_key, value) in map {
        match value {
          serde_json::Value::String(value) if value.starts_with("${") && value.ends_with('}') => {
            // Check if the value is a placeholder for provided data
            {
              let var_name = &value[2..value.len() - 1];
              context_variables.push(var_name.to_string());
            }
          }
          _ => visit(value, context_variables),
        }
      }
    }
  }

  let mut context_variables = vec![];
  visit(meta, &mut context_variables);
  context_variables
}

pub fn inject_context_data(meta: &mut serde_json::Value, context_data: &FxHashMap<String, String>) {
  if let serde_json::Value::Object(map) = meta {
    for value in map.values_mut() {
      if let serde_json::Value::String(value) = value {
        if value.starts_with("${") && value.ends_with('}') {
          // Check if the value is a placeholder for provided data
          let var_name = &value[2..value.len() - 1];
          if let Some(replacement_value) = context_data.get(var_name) {
            *value = replacement_value.clone();
          }
        }
      }
    }
  }
}
