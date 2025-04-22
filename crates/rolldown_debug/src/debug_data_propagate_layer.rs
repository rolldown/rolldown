use std::{ops::Deref, sync::Arc};

use rustc_hash::FxHashMap;
use tracing::{Subscriber, span::Attributes};
use tracing_subscriber::{Layer, layer::Context, registry::LookupSpan};

#[derive(Debug, Clone)]

pub struct DebugDataPropagateLayer;

#[derive(Debug, Clone, serde::Serialize)]
pub struct SessionId(pub Arc<str>);

#[derive(Debug)]
pub struct ProvidedData(FxHashMap<String, String>);

impl Deref for ProvidedData {
  type Target = FxHashMap<String, String>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

#[derive(Default)]
pub struct ProvidedDataFinder {
  provided_data: FxHashMap<String, String>,
}

const PROVIDE_PREFIX: &str = "PROVIDE_";
const PROVIDE_PREFIX_LEN: usize = PROVIDE_PREFIX.len();
impl tracing::field::Visit for ProvidedDataFinder {
  fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
    if field.name().starts_with("PROVIDE") {
      let key = field.name()[PROVIDE_PREFIX_LEN..].to_string();
      let value = value.to_string();
      self.provided_data.insert(key, value);
    }
  }
  fn record_debug(&mut self, _: &tracing::field::Field, _: &dyn std::fmt::Debug) {
    // Ignore debug fields
  }
}

struct DebugDataFinder {
  session_id: Option<Arc<str>>,
}
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

    let mut provided_data_finder = ProvidedDataFinder::default();
    attrs.record(&mut provided_data_finder);
    let mut exts = span_ref.extensions_mut();
    if !provided_data_finder.provided_data.is_empty() {
      exts.insert(ProvidedData(provided_data_finder.provided_data));
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
