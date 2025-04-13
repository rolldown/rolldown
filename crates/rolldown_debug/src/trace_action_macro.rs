#[macro_export]
macro_rules! trace_action {
  ($expr:expr) => {
    tracing::trace!(action = $expr.as_value());
  };
}
