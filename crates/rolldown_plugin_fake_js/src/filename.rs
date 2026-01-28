use regex::Regex;
use std::sync::LazyLock;

static RE_DTS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\.d\.(ts|mts|cts)$").unwrap());

pub fn is_dts(filename: &str) -> bool {
  RE_DTS.is_match(filename)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_is_dts() {
    assert!(is_dts("foo.d.ts"));
    assert!(is_dts("foo.d.mts"));
    assert!(is_dts("foo.d.cts"));
    assert!(!is_dts("foo.ts"));
    assert!(!is_dts("foo.js"));
  }
}
