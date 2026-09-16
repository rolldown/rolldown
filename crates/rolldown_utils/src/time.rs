/// Sleep until `deadline`.
///
/// Resolves at/after `deadline` and cancels the underlying timer when dropped —
/// the `select!` losing-arm semantics the watch coordinator's debounce loop
/// relies on.
pub fn sleep_until(deadline: std::time::Instant) -> crate::async_runtime::Sleep {
  crate::async_runtime::sleep_until(deadline)
}
