/// Sleep until `deadline` (shared-runtime flavor).
///
/// Both flavors resolve at/after `deadline` and cancel the underlying timer
/// when dropped — the `tokio::select!` losing-arm semantics the watch
/// coordinator's debounce loop relies on.
#[cfg(not(feature = "tokio-runtime"))]
pub fn sleep_until(deadline: std::time::Instant) -> crate::async_runtime::Sleep {
  crate::async_runtime::sleep_until(deadline)
}

/// Sleep until `deadline` (tokio flavor; see the `async-runtime` variant above).
#[cfg(feature = "tokio-runtime")]
pub fn sleep_until(deadline: std::time::Instant) -> tokio::time::Sleep {
  tokio::time::sleep_until(deadline.into())
}
