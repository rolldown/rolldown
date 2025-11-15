#[macro_export]
macro_rules! trace_action {
  ($expr:expr) => {
    tracing::trace!(devtoolsAction = serde_json::to_string(&$expr).unwrap());
  };
}

#[macro_export]
macro_rules! trace_action_enabled {
  () => {
    tracing::enabled!(tracing::Level::TRACE)
  };
}
