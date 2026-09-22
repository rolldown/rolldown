/// Sleep until `deadline`.
///
/// Resolves at/after `deadline` and cancels the underlying timer when dropped —
/// the `select!` losing-arm semantics the watch coordinator's debounce loop
/// relies on.
pub use crate::async_runtime::sleep_until;
