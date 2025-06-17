use std::{ops::Deref, sync::Arc};

use tracing::{Subscriber, span::Attributes};
use tracing_subscriber::{Layer, layer::Context, registry::LookupSpan};

use crate::type_alias::ContextDataMap;

#[derive(Debug, Clone)]

pub struct DebugDataPropagateLayer;

struct DebugDataFinder {
  session_id: Option<Arc<str>>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SessionId(pub Arc<str>);

impl tracing::field::Visit for DebugDataFinder {
  fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
    if field.name() == "session_id" {
      self.session_id = Some(value.into());
    }
  }
  /// Visit an unsigned 128-bit integer value.
  fn record_u128(&mut self, field: &tracing::field::Field, value: u128) {
    if field.name() == "session_id" {
      self.session_id = Some(value.to_string().into());
    }
  }
  fn record_debug(&mut self, _: &tracing::field::Field, _: &dyn std::fmt::Debug) {
    // Ignore debug fields
  }
}

impl<S> Layer<S> for DebugDataPropagateLayer
where
  S: Subscriber + for<'a> LookupSpan<'a>,
{
  fn on_new_span(&self, attrs: &Attributes<'_>, id: &tracing::Id, ctx: Context<'_, S>) {
    let Some(span_ref) = ctx.span(id) else {
      return;
    };

    let mut visitor = DebugDataFinder { session_id: None };
    // First see if the current span has a `buildId` field
    attrs.record(&mut visitor);

    let mut context_data_finder = ContextDataFinder::default();
    attrs.record(&mut context_data_finder);

    let mut exts = span_ref.extensions_mut();

    if !context_data_finder.context_data.is_empty() {
      exts.insert(ContextData(context_data_finder.context_data));
    }

    if let Some(build_id) = visitor.session_id {
      // If it does, it means this span is the root build span. Let's store the `buildId` into the extensions.
      exts.insert(SessionId(build_id));
    } else {
      // If not, we need to propagate the `buildId` from the parent span.
      let mut next_parent_ref = span_ref.parent();
      let mut ancestors = vec![];
      let build_id = loop {
        // Find the first ancestor that has a `buildId` field
        if let Some(parent_ref) = next_parent_ref {
          if let Some(build_id) = parent_ref.extensions().get::<SessionId>() {
            break Some(build_id.clone());
          }
          next_parent_ref = parent_ref.parent();
          ancestors.push(parent_ref);
        } else {
          break None;
        }
      };

      if let Some(build_id) = build_id {
        // If we found a `buildId` in the parent span, we need to propagate it to the current span.
        exts.insert(build_id.clone());
        // And also propagate it to all the ancestors we visited.
        for ancestor in ancestors {
          ancestor.extensions_mut().insert(build_id.clone());
        }
      }
    }
  }
}

#[derive(Debug)]
pub struct ContextData(ContextDataMap);

impl Deref for ContextData {
  type Target = ContextDataMap;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

#[derive(Default)]
pub struct ContextDataFinder {
  context_data: ContextDataMap,
}

const CONTEXT_PREFIX: &str = "CONTEXT_";
const CONTEXT_PREFIX_LEN: usize = CONTEXT_PREFIX.len();
impl tracing::field::Visit for ContextDataFinder {
  fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
    // Only record context data that starts with `CONTEXT_`.
    if field.name().starts_with(CONTEXT_PREFIX) {
      let key = &field.name()[CONTEXT_PREFIX_LEN..];
      let value = value.to_string();
      self.context_data.insert(key, value);
    }
  }
  fn record_debug(&mut self, _: &tracing::field::Field, _: &dyn std::fmt::Debug) {
    // Ignore debug fields
  }
}
