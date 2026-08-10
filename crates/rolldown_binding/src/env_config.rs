/// Resolve an env-derived thread count, clamped to `maximum`. Missing,
/// non-numeric, or zero all count as "unset" and fall back to `default`: a `0`
/// reaching `configure()` fails `RuntimeOptions::validate()` and panics the
/// module-init `expect()`.
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
    // RD-3: a `0` typo must read as unset; `RuntimeOptions::validate()` rejects 0.
    assert_eq!(resolve_thread_count(Some("0".to_string()), DEFAULT, MAXIMUM), DEFAULT);
    assert_eq!(resolve_thread_count(Some("4".to_string()), DEFAULT, MAXIMUM), 4);
    assert_eq!(resolve_thread_count(Some("1000".to_string()), DEFAULT, MAXIMUM), MAXIMUM);
    assert_eq!(resolve_thread_count(Some(usize::MAX.to_string()), DEFAULT, MAXIMUM), MAXIMUM);
    // Parse overflow falls back instead of wrapping.
    assert_eq!(
      resolve_thread_count(Some("18446744073709551616".to_string()), DEFAULT, MAXIMUM,),
      DEFAULT
    );
    assert_eq!(resolve_thread_count(Some("abc".to_string()), DEFAULT, MAXIMUM), DEFAULT);
    assert_eq!(resolve_thread_count(None, DEFAULT, MAXIMUM), DEFAULT);
    // The host-derived default is clamped too.
    assert_eq!(resolve_thread_count(None, 32, MAXIMUM), MAXIMUM);
  }

  #[test]
  #[should_panic(expected = "the default thread count must be positive")]
  fn resolve_thread_count_rejects_an_invalid_default() {
    let _ = resolve_thread_count(None, 0, 1);
  }
}
