/// Sleep until `deadline`, mirroring `futures.rs`'s facade pattern: the
/// `async-runtime` build routes through the shared runtime's timer facility
/// (MultiThread heap driver / CurrentThread host-delegated driver -- see
/// `async_runtime`'s timer section), every other build through the tokio
/// timer wheel. Both futures resolve at/after `deadline` and cancel their
/// underlying timer when dropped (the `tokio::select!` losing-arm semantics
/// the watch coordinator's debounce loop relies on).
#[cfg(feature = "async-runtime")]
pub fn sleep_until(deadline: std::time::Instant) -> crate::async_runtime::Sleep {
  crate::async_runtime::sleep_until(deadline)
}

/// Sleep until `deadline` (tokio flavor; see the `async-runtime` variant above).
#[cfg(not(feature = "async-runtime"))]
pub fn sleep_until(deadline: std::time::Instant) -> tokio::time::Sleep {
  tokio::time::sleep_until(deadline.into())
}
