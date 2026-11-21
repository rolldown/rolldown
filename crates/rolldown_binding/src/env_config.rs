/// A positive integer thread count; empty, zero, negative, fractional or
/// overflowing values are `None`.
pub fn parse_thread_count(value: &str) -> Option<usize> {
  value.parse::<std::num::NonZeroUsize>().ok().map(std::num::NonZeroUsize::get)
}

/// Resolve an env-derived thread count, clamped to `maximum`. Missing or invalid
/// values (see [`parse_thread_count`]) count as "unset" and fall back to `default`:
/// a `0` reaching `configure()` fails `RuntimeOptions::validate()` and panics the
/// module-init `expect()`.
pub fn resolve_thread_count(raw: Option<String>, default: usize, maximum: usize) -> usize {
  assert!(default > 0, "the default thread count must be positive");
  assert!(maximum > 0, "the maximum thread count must be positive");
  raw.as_deref().and_then(parse_thread_count).unwrap_or(default).min(maximum)
}

#[cfg(test)]
mod tests {
  use super::resolve_thread_count;

  #[test]
  fn resolve_thread_count_handles_env_inputs() {
    const DEFAULT: usize = 8;
    const MAXIMUM: usize = 16;
    // A `0` typo reads as unset.
    assert_eq!(resolve_thread_count(Some("0".to_string()), DEFAULT, MAXIMUM), DEFAULT);
    assert_eq!(resolve_thread_count(Some("4".to_string()), DEFAULT, MAXIMUM), 4);
    assert_eq!(resolve_thread_count(Some("1000".to_string()), DEFAULT, MAXIMUM), MAXIMUM);
    assert_eq!(resolve_thread_count(Some(usize::MAX.to_string()), DEFAULT, MAXIMUM), MAXIMUM);
    // Parse overflow falls back instead of wrapping.
    assert_eq!(
      resolve_thread_count(Some("18446744073709551616".to_string()), DEFAULT, MAXIMUM,),
      DEFAULT
    );
    for invalid in ["abc", "", "-1", "1.5"] {
      assert_eq!(resolve_thread_count(Some(invalid.to_string()), DEFAULT, MAXIMUM), DEFAULT);
    }
    assert_eq!(resolve_thread_count(None, DEFAULT, MAXIMUM), DEFAULT);
    // The host-derived default is clamped too.
    assert_eq!(resolve_thread_count(None, 32, MAXIMUM), MAXIMUM);
  }
}
