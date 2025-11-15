use tracing::{Subscriber, span::Attributes};
use tracing_subscriber::{Layer, layer::Context, registry::LookupSpan};

use crate::types::{ContextData, ContextDataExtractor};

#[derive(Debug, Clone)]
// A tracing layer that extracts context data from spans so events can use them later.
pub struct DevtoolsLayer;

impl DevtoolsLayer {
  fn extract_context_data(attrs: &Attributes<'_>) -> Option<ContextData> {
    let mut context_data_finder = ContextDataExtractor::default();
    attrs.record(&mut context_data_finder);
    if context_data_finder.context_data.is_empty() {
      None
    } else {
      Some(ContextData(context_data_finder.context_data))
    }
  }
}

impl<S> Layer<S> for DevtoolsLayer
where
  S: Subscriber + for<'a> LookupSpan<'a>,
{
  fn on_new_span(&self, attrs: &Attributes<'_>, id: &tracing::Id, ctx: Context<'_, S>) {
    if let Some(span_ref) = ctx.span(id) {
      let context_data = Self::extract_context_data(attrs);
      if let Some(context_data) = context_data {
        let mut exts = span_ref.extensions_mut();
        exts.insert(context_data);
      }
    }
  }
}
