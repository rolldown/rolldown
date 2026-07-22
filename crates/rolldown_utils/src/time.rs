/// Sleep until `deadline`, routed through the shared runtime's timer facility
/// (MultiThread heap driver / CurrentThread host-delegated driver -- see
/// `async_runtime`'s timer section). The future resolves at/after `deadline`
/// and cancels its underlying timer when dropped (the select! losing-arm
/// semantics the watch coordinator's debounce loop relies on).
pub fn sleep_until(deadline: std::time::Instant) -> crate::async_runtime::Sleep {
  crate::async_runtime::sleep_until(deadline)
}
