/// Resolve an env-derived thread count: parse the raw value, treat a missing,
/// non-numeric, OR zero value as "unset" and fall back to `default`. A `0` must
/// never survive here because both module-init paths feed this count into a
/// constructor that rejects `0`:
/// - `register_async_runtime` (async-runtime feature) — `RuntimeOptions::validate()`
///   rejects a `0` thread count, panicking the `expect()` during addon load.
/// - `init` (default tokio-runtime feature) — `tokio::runtime::Builder`'s
///   `worker_threads(0)` / `max_blocking_threads(0)` assert `> 0` and panic the
///   `expect()` during addon load.
///
/// Behaviour-identical to the previous inline `.parse::<usize>().ok().unwrap_or(default)`
/// for every non-zero numeric input and for non-numeric / unset values.
pub fn resolve_thread_count(raw: Option<String>, default: usize) -> usize {
  raw
    .and_then(|value| value.parse::<usize>().ok())
    .filter(|&value| value != 0)
    .unwrap_or(default)
}

#[cfg(test)]
mod tests {
  use super::resolve_thread_count;

  #[test]
  fn resolve_thread_count_handles_env_inputs() {
    const DEFAULT: usize = 8;
    // RD-3: a `0` typo must be treated as unset, never fed to a constructor that
    // rejects 0 (validate() / tokio Builder), which would panic the module_init
    // `expect`.
    assert_eq!(resolve_thread_count(Some("0".to_string()), DEFAULT), DEFAULT);
    // Valid positive numbers pass through unchanged.
    assert_eq!(resolve_thread_count(Some("4".to_string()), DEFAULT), 4);
    // Non-numeric values fall back gracefully (unchanged behaviour).
    assert_eq!(resolve_thread_count(Some("abc".to_string()), DEFAULT), DEFAULT);
    // Unset falls back to the default.
    assert_eq!(resolve_thread_count(None, DEFAULT), DEFAULT);
  }
}
