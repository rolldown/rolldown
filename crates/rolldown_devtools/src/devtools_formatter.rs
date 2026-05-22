use std::sync::Arc;

use crate::{
  static_data::DEFAULT_SESSION_ID,
  types::{ContextData, DevtoolsActionFieldExtractor},
  writer::{self, LogCommand},
};
use rustc_hash::{FxHashMap, FxHashSet};
use tracing::{Event, Subscriber};
use tracing_subscriber::{
  fmt::{FmtContext, FormatEvent, FormatFields, format::Writer},
  registry::LookupSpan,
};

/// A formatter that formats tracing events into devtools compatible JSON lines
/// and hands them to the dedicated writer thread. The on-hot-path work is just
/// JSON shaping + a channel send; no file I/O, no cross-thread locks.
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
    let Some(mut action_value) = Self::extract_action(event) else {
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

    let session_id = found_context_fields
      .get("session_id")
      .map_or_else(|| DEFAULT_SESSION_ID.to_string(), String::to_owned);

    let is_session_meta = action_value
      .as_object()
      .expect("action_meta should always be an object")
      .get("action")
      .is_some_and(|v| v == "SessionMeta");

    let filename: Arc<str> = if is_session_meta {
      format!("node_modules/.rolldown/{session_id}/meta.json").into()
    } else {
      format!("node_modules/.rolldown/{session_id}/logs.json").into()
    };

    writer::send(LogCommand::Write { session_id, filename, action_value });
    Ok(())
  }
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
