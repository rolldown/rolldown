use std::{
  fs::OpenOptions,
  io::Write,
  sync::Arc,
  time::{SystemTime, UNIX_EPOCH},
};

use crate::{
  debug_data_propagate_layer::ContextData,
  static_data::{
    DEFAULT_SESSION_ID, EXIST_HASH_BY_SESSION, OPENED_FILE_HANDLES, OPENED_FILES_BY_SESSION,
  },
};
use rustc_hash::{FxHashMap, FxHashSet};
use serde::ser::{SerializeMap, Serializer as _};
use tracing::{Event, Subscriber};
use tracing_subscriber::{
  fmt::{FmtContext, FormatEvent, FormatFields, format::Writer},
  registry::LookupSpan,
};

pub struct DebugFormatter;

impl<S, N> FormatEvent<S, N> for DebugFormatter
where
  S: Subscriber + for<'lookup> LookupSpan<'lookup>,
  N: for<'writer> FormatFields<'writer> + 'static,
{
  #[expect(clippy::too_many_lines)]
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
      let action_meta_as_object = action_meta.as_object_mut().expect("meta must be an object");

      // Push `${build_id}` and `${session_id}` to every event, make build id get injected automatically via the context-inject mechanism.
      action_meta_as_object.insert("build_id".to_string(), "${build_id}".into());
      action_meta_as_object.insert("session_id".to_string(), "${session_id}".into());

      let mut contextual_fields = extract_fields_relied_on_context_data(&action_meta);
      let mut captured_values = FxHashMap::default();

      if let Some(scope) = ctx.event_scope() {
        for span in scope {
          if contextual_fields.is_empty() {
            // If there are no contextual fields to inject, we can break early.
            break;
          }
          let span_extensions = span.extensions();
          let Some(context_data) = span_extensions.get::<ContextData>() else {
            continue;
          };
          contextual_fields.retain(|field_name| {
            if let Some(value) = context_data.get(field_name.as_str()) {
              captured_values.insert(field_name.clone(), value.clone());
              // Remove the field that has found its value.
              false
            } else {
              true
            }
          });
        }
      }

      inject_context_data(&mut action_meta, &captured_values);

      let session_id =
        captured_values.get("session_id").map(String::as_str).unwrap_or(DEFAULT_SESSION_ID);

      std::fs::create_dir_all(format!(".rolldown/{session_id}")).ok();

      let is_session_meta = action_meta
        .as_object()
        .expect("action_meta should always be an object")
        .get("action")
        .is_some_and(|v| v == "SessionMeta");

      let log_filename: Arc<str> = if is_session_meta {
        format!(".rolldown/{session_id}/meta.json").into()
      } else {
        format!(".rolldown/{session_id}/logs.json").into()
      };

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

      let mut need_newline = false;
      let mut cache_large_string = || -> Result<(), serde_json::Error> {
        // WARN: Do not use pretty print here, vite-devtool relies on the format of every line is a json object.
        let mut serializer = serde_json::Serializer::new(&mut file);

        let serde_json::Value::Object(action_meta) = &action_meta else {
          unreachable!("action_meta should always be an object");
        };

        for (_key, value) in action_meta {
          match value {
            serde_json::Value::String(value) if value.len() > 5 * 1024 /* 5kb */ => {
              // we assume hash does not collide.
              let hash = blake3::hash(value.as_bytes()).to_hex().to_string();
              let mut exist_hash_set =
                EXIST_HASH_BY_SESSION.entry(session_id.to_string()).or_default();
              if !exist_hash_set.contains(&hash) {
                exist_hash_set.insert(hash.to_string());
                let mut map = serializer.serialize_map(None)?;
                map.serialize_entry("action", "StringRef")?;
                map.serialize_entry("id", &hash)?;
                map.serialize_entry("content", value)?;
                map.end()?;
                need_newline = true;
              }
            }
            _ => {
            }
          }
        }
        Ok(())
      };

      cache_large_string().map_err(|_| std::fmt::Error)?;
      if need_newline {
        writeln!(file).map_err(|_| std::fmt::Error)?;
      }

      let mut visit = || {
        // WARN: Do not use pretty print here, vite-devtool relies on the format of every line is a json object.
        let mut serializer = serde_json::Serializer::new(&mut file);

        let serde_json::Value::Object(action_meta) = &action_meta else {
          unreachable!("action_meta should always be an object");
        };

        for (_key, value) in action_meta {
          match value {
            serde_json::Value::String(value) if value.len() > 5 * 1024 /* 5kb */ => {
              // we assume hash does not collide.
              let hash = blake3::hash(value.as_bytes()).to_hex().to_string();
              let mut exist_hash_set =
                EXIST_HASH_BY_SESSION.entry(session_id.to_string()).or_default();
              if !exist_hash_set.contains(&hash) {
                exist_hash_set.insert(hash.to_string());
                let mut map = serializer.serialize_map(None)?;
                map.serialize_entry("action", "StringRef")?;
                map.serialize_entry("id", &hash)?;
                map.serialize_entry("content", value)?;
                map.end()?;
              }
            }
            _ => {
            }
          }
        }

        let mut serializer = serializer.serialize_map(None)?;

        serializer.serialize_entry("timestamp", &current_utc_timestamp_ms())?;

        for (key, value) in action_meta {
          match value {
            serde_json::Value::String(value) if value.len() > 10 * 1024 /* 10kb */ => {
              // we assume hash does not collide.
              let hash = blake3::hash(value.as_bytes()).to_hex().to_string();
              serializer.serialize_entry(key, &format!("$ref:{hash}"))?;
            }
            _ => {
              serializer.serialize_entry(key, value)?;
            }
          }
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

pub fn extract_fields_relied_on_context_data(meta: &serde_json::Value) -> FxHashSet<String> {
  fn visit(meta: &serde_json::Value, context_variables: &mut FxHashSet<String>) {
    if let serde_json::Value::Object(map) = meta {
      for (_key, value) in map {
        match value {
          serde_json::Value::String(value) if value.starts_with("${") && value.ends_with('}') => {
            // Check if the value is a placeholder for provided data
            {
              let var_name = &value[2..value.len() - 1];
              context_variables.insert(var_name.to_string());
            }
          }
          _ => visit(value, context_variables),
        }
      }
    }
  }

  let mut contextual_fields = FxHashSet::default();
  visit(meta, &mut contextual_fields);
  contextual_fields
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
