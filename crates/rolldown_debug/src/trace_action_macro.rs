#[macro_export]
macro_rules! trace_action {
  ($expr:expr, $($rest:tt)*) => {
    tracing::trace!(action = serde_json::to_string(&$expr).unwrap(), $($rest)*);
  };
  ($expr:expr) => {
    tracing::trace!(action = serde_json::to_string(&$expr).unwrap());
  };
}
