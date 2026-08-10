use std::sync::LazyLock;

use regex::Regex;

static RE_DTS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\.d\.(ts|mts|cts)$").unwrap());

pub fn is_dts(filename: &str) -> bool {
  RE_DTS.is_match(filename)
}

pub fn patch_dts_extension(source: &str) -> String {
  if let Some(base) = source.strip_suffix(".d.ts") {
    format!("{base}.js")
  } else if let Some(base) = source.strip_suffix(".d.mts") {
    format!("{base}.mjs")
  } else if let Some(base) = source.strip_suffix(".d.cts") {
    format!("{base}.cjs")
  } else {
    source.to_string()
  }
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
