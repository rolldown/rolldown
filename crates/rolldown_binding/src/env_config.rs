/// Resolve an env-derived thread count: parse the raw value, treat a missing,
/// non-numeric, OR zero value as "unset" and fall back to `default`, then clamp
/// the result to `maximum`. A `0` must never survive here because module init
/// feeds this count into `register_async_runtime`, whose
/// `RuntimeOptions::validate()` rejects a `0` thread count, panicking the
/// `expect()` during addon load.
pub fn resolve_thread_count(raw: Option<String>, default: usize, maximum: usize) -> usize {
  assert!(default > 0, "the default thread count must be positive");
  assert!(maximum > 0, "the maximum thread count must be positive");
  raw
    .and_then(|value| value.parse::<usize>().ok())
    .filter(|&value| value != 0)
    .unwrap_or(default)
    .min(maximum)
}

#[cfg(test)]
mod tests {
  use super::resolve_thread_count;

  #[test]
  fn resolve_thread_count_handles_env_inputs() {
    const DEFAULT: usize = 8;
    const MAXIMUM: usize = 16;
    // RD-3: a `0` typo must be treated as unset, never fed to a constructor that
    // rejects 0 (`RuntimeOptions::validate()`), which would panic the
    // module_init `expect`.
    assert_eq!(resolve_thread_count(Some("0".to_string()), DEFAULT, MAXIMUM), DEFAULT);
    // Valid positive numbers pass through unchanged.
    assert_eq!(resolve_thread_count(Some("4".to_string()), DEFAULT, MAXIMUM), 4);
    // Valid but unsafe values clamp to the construction limit.
    assert_eq!(resolve_thread_count(Some("1000".to_string()), DEFAULT, MAXIMUM), MAXIMUM);
    assert_eq!(resolve_thread_count(Some(usize::MAX.to_string()), DEFAULT, MAXIMUM), MAXIMUM);
    // Parse overflow is invalid input and falls back instead of wrapping.
    assert_eq!(
      resolve_thread_count(Some("18446744073709551616".to_string()), DEFAULT, MAXIMUM,),
      DEFAULT
    );
    // Non-numeric values fall back gracefully (unchanged behaviour).
    assert_eq!(resolve_thread_count(Some("abc".to_string()), DEFAULT, MAXIMUM), DEFAULT);
    // Unset falls back to the default.
    assert_eq!(resolve_thread_count(None, DEFAULT, MAXIMUM), DEFAULT);
    // Host-derived defaults are subject to the same production ceiling.
    assert_eq!(resolve_thread_count(None, 32, MAXIMUM), MAXIMUM);
  }

  #[test]
  #[should_panic(expected = "the default thread count must be positive")]
  fn resolve_thread_count_rejects_an_invalid_default() {
    let _ = resolve_thread_count(None, 0, 1);
  }
}
