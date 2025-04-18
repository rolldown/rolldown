use std::sync::Arc;

use tracing::{Subscriber, span::Attributes};
use tracing_subscriber::{Layer, layer::Context, registry::LookupSpan};

#[derive(Debug, Clone)]

pub struct DebugDataPropagateLayer;

struct DebugDataFinder {
  build_id: Option<Arc<str>>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct BuildId(pub Arc<str>);

impl tracing::field::Visit for DebugDataFinder {
  fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
    if field.name() == "buildId" {
      self.build_id = Some(value.into());
    }
  }
  /// Visit an unsigned 128-bit integer value.
  fn record_u128(&mut self, field: &tracing::field::Field, value: u128) {
    if field.name() == "buildId" {
      self.build_id = Some(value.to_string().into());
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

    let mut visitor = DebugDataFinder { build_id: None };
    // First see if the current span has a `buildId` field
    attrs.record(&mut visitor);

    let mut exts = span_ref.extensions_mut();
    if let Some(build_id) = visitor.build_id {
      // If it does, it means this span is the root build span. Let's store the `buildId` into the extensions.
      exts.insert(BuildId(build_id));
    } else {
      // If not, we need to propagate the `buildId` from the parent span.
      let mut next_parent_ref = span_ref.parent();
      let mut ancestors = vec![];
      let build_id = loop {
        // Find the first ancestor that has a `buildId` field
        if let Some(parent_ref) = next_parent_ref {
          if let Some(build_id) = parent_ref.extensions().get::<BuildId>() {
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
