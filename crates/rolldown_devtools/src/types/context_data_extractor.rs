use crate::type_alias::ContextDataMap;

const CONTEXT_PREFIX: &str = "CONTEXT_";
const CONTEXT_PREFIX_LEN: usize = CONTEXT_PREFIX.len();

#[derive(Default)]
pub struct ContextDataExtractor {
  pub(crate) context_data: ContextDataMap,
}

impl tracing::field::Visit for ContextDataExtractor {
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
