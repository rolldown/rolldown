use std::{
  fs::OpenOptions,
  io::Write,
  sync::Arc,
  time::{SystemTime, UNIX_EPOCH},
};

use crate::{
  static_data::{
    DEFAULT_SESSION_ID, EXIST_HASH_BY_SESSION, OPENED_FILE_HANDLES, OPENED_FILES_BY_SESSION,
  },
  types::{ContextData, DevtoolsActionFieldExtractor},
};
use rustc_hash::{FxHashMap, FxHashSet};
use serde::ser::{SerializeMap, Serializer as _};
use tracing::{Event, Subscriber};
use tracing_subscriber::{
  fmt::{FmtContext, FormatEvent, FormatFields, format::Writer},
  registry::LookupSpan,
};

/// A formatter that formats tracing events into devtools compatible JSON lines and writes them into files.
pub struct DevtoolsFormatter;

impl DevtoolsFormatter {
  fn extract_action(event: &Event<'_>) -> Option<serde_json::Value> {
    let mut action_meta_extractor = DevtoolsActionFieldExtractor::default();
    event.record(&mut action_meta_extractor);
    action_meta_extractor.value
  }
}

impl<S, N> FormatEvent<S, N> for DevtoolsFormatter
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
    let action_value = Self::extract_action(event);
    let Some(mut action_value) = action_value else {
      // This branch means this event is not for devtools tracing.
      return Ok(());
    };

    let action_value_as_object = action_value.as_object_mut().expect("meta must be an object");

    // Push `${build_id}` and `${session_id}` to every event, make build id get injected automatically via the context-inject mechanism.
    action_value_as_object.insert("build_id".to_string(), "${build_id}".into());
    action_value_as_object.insert("session_id".to_string(), "${session_id}".into());

    let mut contextual_variable = extract_context_variables_from_action(&action_value);
    let mut found_context_fields = FxHashMap::default();

    if let Some(scope) = ctx.event_scope() {
      for span in scope {
        let span_extensions = span.extensions();
        let Some(context_data) = span_extensions.get::<ContextData>() else {
          continue;
        };
        contextual_variable.retain(|var_name| {
          if let Some(value) = context_data.get(var_name.as_str()) {
            found_context_fields.insert(var_name.clone(), value.clone());
            // Remove the field that has found its value.
            false
          } else {
            true
          }
        });
        if contextual_variable.is_empty() {
          break;
        }
      }
    }

    inject_context_data(&mut action_value, &found_context_fields);

    let session_id =
      found_context_fields.get("session_id").map_or(DEFAULT_SESSION_ID, String::as_str);

    std::fs::create_dir_all(format!("node_modules/.rolldown/{session_id}")).ok();

    let is_session_meta = action_value
      .as_object()
      .expect("action_meta should always be an object")
      .get("action")
      .is_some_and(|v| v == "SessionMeta");

    let log_filename: Arc<str> = if is_session_meta {
      format!("node_modules/.rolldown/{session_id}/meta.json").into()
    } else {
      format!("node_modules/.rolldown/{session_id}/logs.json").into()
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

      let serde_json::Value::Object(action_meta) = &action_value else {
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
                exist_hash_set.insert(hash.clone());
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

      let serde_json::Value::Object(action_meta) = &action_value else {
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
                exist_hash_set.insert(hash.clone());
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
  }
}

fn current_utc_timestamp_ms() -> u128 {
  SystemTime::now().duration_since(UNIX_EPOCH).expect("Time went backwards").as_millis()
}

// For prop value pair like: `id: "${call_id}"`, extract `call_id` so we can look up its value from context data.
pub fn extract_context_variables_from_action(meta: &serde_json::Value) -> FxHashSet<String> {
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
