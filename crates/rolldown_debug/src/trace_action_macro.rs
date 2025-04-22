#[macro_export]
macro_rules! trace_action {
  ($expr:expr) => {
    tracing::trace!(meta = serde_json::to_string(&$expr).unwrap());
  };
}
