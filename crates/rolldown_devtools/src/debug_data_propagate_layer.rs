use std::ops::Deref;

use tracing::{Subscriber, span::Attributes};
use tracing_subscriber::{Layer, layer::Context, registry::LookupSpan};

use crate::type_alias::ContextDataMap;

#[derive(Debug, Clone)]

pub struct DebugDataPropagateLayer;

impl<S> Layer<S> for DebugDataPropagateLayer
where
  S: Subscriber + for<'a> LookupSpan<'a>,
{
  fn on_new_span(&self, attrs: &Attributes<'_>, id: &tracing::Id, ctx: Context<'_, S>) {
    let Some(span_ref) = ctx.span(id) else {
      return;
    };

    let mut context_data_finder = ContextDataFinder::default();
    attrs.record(&mut context_data_finder);

    let mut exts = span_ref.extensions_mut();

    if !context_data_finder.context_data.is_empty() {
      exts.insert(ContextData(context_data_finder.context_data));
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
