/// Extracts `devtoolsAction` field from tracing events.
#[derive(Default)]
pub struct DevtoolsActionFieldExtractor {
  pub value: Option<serde_json::Value>,
}

impl tracing::field::Visit for DevtoolsActionFieldExtractor {
  fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
    if field.name() == "devtoolsAction" {
      self.value = Some(serde_json::from_str(value).unwrap());
    }
  }

  fn record_debug(&mut self, _field: &tracing::field::Field, _value: &dyn std::fmt::Debug) {
    // `EventMetaExtractor` doesn't care about debug values.
  }
}
