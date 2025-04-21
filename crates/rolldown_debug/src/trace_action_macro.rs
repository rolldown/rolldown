#[macro_export]
macro_rules! trace_action {
  (true, $expr:expr) => {
    tracing::trace!(is_meta = true, action = $expr.as_value());
  };
  ($expr:expr) => {
    tracing::trace!(action = $expr.as_value());
  };
}
