use std::borrow::Cow;

use crate::concat_string;

/// The fast path uses `regex-lite`, a much smaller, dependency-free engine than the
/// full `regex` crate (no `regex-automata`/`regex-syntax`/`aho-corasick`/SIMD `memchr`
/// or large Unicode tables), which meaningfully reduces the shipped binary size.
/// It is a linear-time PikeVM with a larger constant factor than `regex`, and supports
/// a subset of `regex`'s syntax. Patterns it cannot compile fall back to `regress`.
///
/// According to the doc of `regress`, https://docs.rs/regress/0.10.0/regress/#comparison-to-regex-crate
/// **regress supports features that regex does not, in particular backreferences and zero-width lookaround assertions.**
/// these features are not commonly used, so in most cases the slow path will not be reached.
#[derive(Debug, Clone)]
pub enum HybridRegex {
  Optimize(regex_lite::Regex),
  Ecma(regress::Regex),
}

/// Returns `true` when a JS regex must be evaluated by `regress` (the `Ecma` path) instead
/// of the `regex-lite` (`Optimize`) fast path, because its semantics depend on Unicode.
///
/// `regex-lite` is ASCII-only: it implements ASCII `\s`/`\S` and ASCII case-insensitive
/// matching, so routing Unicode-sensitive patterns to it would produce non-JS results
/// (e.g. `/\s/` not matching NBSP, `/é/i` not matching `É`). Over-routing to `regress` is
/// safe (it is the correct JS engine); the binary-size win is unaffected because it comes
/// from which crates are *linked*, not from runtime routing.
///
/// Note `\w`/`\d`/`\b` are ASCII in JS by definition, so patterns using only those stay on
/// the `regex-lite` fast path (which is actually more JS-correct there than the old `regex`).
fn needs_unicode_engine(pattern: &str, flags: &str) -> bool {
  // The `i` (ignoreCase) flag triggers Unicode case folding in JS; `regex-lite` is ASCII-only
  // (so `/é/i` matching `É` requires `regress`).
  if flags.contains('i') {
    return true;
  }

  // A non-ASCII literal codepoint can interact with case folding / Unicode literals; route to
  // `regress` to be safe.
  if !pattern.is_ascii() {
    return true;
  }

  let bytes = pattern.as_bytes();
  let mut i = 0;
  while i < bytes.len() {
    if bytes[i] == b'\\' && i + 1 < bytes.len() {
      match bytes[i + 1] {
        // `\s`/`\S`: JS whitespace includes Unicode (NBSP, ` `, etc.); `regex-lite` is ASCII.
        // `\p{...}`/`\P{...}`: Unicode property classes; `regex-lite` cannot compile them anyway.
        b's' | b'S' | b'p' | b'P' => return true,
        // Skip the escaped character so an escaped backslash (`\\`) is not misread.
        _ => {
          i += 2;
          continue;
        }
      }
    }
    i += 1;
  }

  false
}

// Please only used for testing
impl From<&str> for HybridRegex {
  fn from(pattern: &str) -> Self {
    HybridRegex::new(pattern).unwrap_or_else(|err| {
      panic!("failed to create HybridRegex from {pattern}, error details: {err}")
    })
  }
}
impl HybridRegex {
  pub fn new(pattern: &str) -> anyhow::Result<Self> {
    if needs_unicode_engine(pattern, "") {
      return regress::Regex::new(pattern).map(HybridRegex::Ecma).map_err(anyhow::Error::from);
    }
    let regex_pattern = Self::get_regex_pattern(pattern, "");
    match regex_lite::Regex::new(&regex_pattern).map(HybridRegex::Optimize) {
      Ok(reg) => Ok(reg),
      Err(_) => regress::Regex::new(pattern).map(HybridRegex::Ecma).map_err(anyhow::Error::from),
    }
  }

  pub fn with_flags(pattern: &str, flags: &str) -> anyhow::Result<Self> {
    if needs_unicode_engine(pattern, flags) {
      return regress::Regex::with_flags(pattern, flags)
        .map(HybridRegex::Ecma)
        .map_err(anyhow::Error::from);
    }
    let regex_pattern = Self::get_regex_pattern(pattern, flags);
    match regex_lite::Regex::new(&regex_pattern).map(HybridRegex::Optimize) {
      Ok(reg) => Ok(reg),
      Err(_) => regress::Regex::with_flags(pattern, flags)
        .map(HybridRegex::Ecma)
        .map_err(anyhow::Error::from),
    }
  }

  pub fn regex_pattern(&self) -> Option<&str> {
    match self {
      HybridRegex::Optimize(r) => Some(r.as_str()),
      HybridRegex::Ecma(_) => None,
    }
  }

  fn get_regex_pattern(pattern: &str, flags: &str) -> String {
    // ECMAScript regex treats CRLF as a line break like LF
    // (e.g. `/a$/m.test("a\r\n")` and `/a$/m.test("a\n")` both returns `true`)
    // Also when `s` flag is not used, `.` does not match CRLF like LF does not.
    concat_string!("(?R", flags, ")", pattern)
  }

  pub fn matches(&self, text: &str) -> bool {
    match self {
      HybridRegex::Optimize(reg) => reg.is_match(text),
      HybridRegex::Ecma(reg) => reg.find(text).is_some(),
    }
  }

  pub fn replace<'a>(&self, haystack: &'a str, replacement: &str) -> Cow<'a, str> {
    match self {
      HybridRegex::Optimize(r) => r.replace(haystack, replacement),
      HybridRegex::Ecma(reg) => {
        // `regress` uses regex-crate-style replacement tokens, not full
        // `String.prototype.replace` semantics. Numbered captures match JS and
        // are what Vite aliases rely on.
        if reg.find(haystack).is_none() {
          return Cow::Borrowed(haystack);
        }
        Cow::Owned(reg.replace(haystack, replacement))
      }
    }
  }

  pub fn replace_all<'a>(&self, haystack: &'a str, replacement: &str) -> Cow<'a, str> {
    match self {
      HybridRegex::Optimize(r) => r.replace_all(haystack, replacement),
      HybridRegex::Ecma(reg) => {
        if reg.find(haystack).is_none() {
          return Cow::Borrowed(haystack);
        }
        Cow::Owned(reg.replace_all(haystack, replacement))
      }
    }
  }
}

#[cfg(test)]
mod test {
  use crate::js_regex::HybridRegex;

  #[test]
  fn with_flags() {
    let reg = HybridRegex::with_flags("a", "i").unwrap();
    assert!(reg.matches("A"));

    let reg = HybridRegex::new("a").unwrap();
    assert!(!reg.matches("A"));
  }

  #[test]
  fn regress_replace_all() {
    let reg = HybridRegex::new(r"\d+(?!\d)").unwrap();
    assert!(matches!(reg, HybridRegex::Ecma(_)));
    assert_eq!(reg.replace_all("111aa111", "1"), "1aa1");
  }

  #[test]
  fn ecma_replace_expands_replacement_tokens() {
    let reg = HybridRegex::new(r"^@app(?!/(?:excluded))(/.*)?$").unwrap();
    assert!(matches!(reg, HybridRegex::Ecma(_)));
    assert_eq!(reg.replace("@app/utils", "/abs/src/app$1"), "/abs/src/app/utils");

    let reg = HybridRegex::new(r"(foo)(?!bar)").unwrap();
    assert!(matches!(reg, HybridRegex::Ecma(_)));
    assert_eq!(reg.replace("foo baz", "$0:$$:$1:$2"), "foo:$:foo: baz");
  }

  #[test]
  fn ecma_replace_all_expands_replacement_tokens() {
    let reg = HybridRegex::new(r"(\d+)(?=px)").unwrap();
    assert!(matches!(reg, HybridRegex::Ecma(_)));
    assert_eq!(reg.replace_all("10px 20px", "$1rem"), "10rempx 20rempx");
  }

  #[test]
  fn js_regex_compat_dot() {
    let dot_reg = HybridRegex::new(".").unwrap();
    assert!(!dot_reg.matches("\n"));
    assert!(!dot_reg.matches("\r\n"));
    // assert!(!dot_reg.matches("\u{2028}")); // FIXME: LINE SEPARATOR should not match
    // assert!(!dot_reg.matches("\u{2029}")); // FIXME: PARAGRAPH SEPARATOR should not match
    let dots_reg = HybridRegex::with_flags(".", "s").unwrap();
    assert!(dots_reg.matches("\n"));
    assert!(dots_reg.matches("\r\n"));
    assert!(dots_reg.matches("\u{2028}"));
    assert!(dots_reg.matches("\u{2029}"));
  }

  #[test]
  fn js_regex_compat_multiline() {
    let reg = HybridRegex::with_flags("a$", "m").unwrap();
    assert!(reg.matches("a\n"));
    assert!(reg.matches("a\r\n"));
  }

  // `\s` in JS matches Unicode whitespace such as NBSP; `regex-lite` only matches ASCII
  // whitespace, so the pattern must be routed to `regress`.
  #[test]
  fn js_regex_unicode_whitespace_routes_to_regress() {
    let reg = HybridRegex::new(r"\s").unwrap();
    assert!(matches!(reg, HybridRegex::Ecma(_)));
    assert!(reg.matches("\u{00A0}"));
  }

  // `/é/i` matches `É` in JS via Unicode case folding; `regex-lite` is ASCII-only, so the
  // `i` flag (and the non-ASCII literal) must route to `regress`.
  #[test]
  fn js_regex_unicode_case_insensitive_routes_to_regress() {
    let reg = HybridRegex::with_flags("é", "i").unwrap();
    assert!(matches!(reg, HybridRegex::Ecma(_)));
    assert!(reg.matches("É"));
  }

  // ASCII-safe patterns keep the `regex-lite` fast path (`\w`/`\d`/`\b` are ASCII in JS).
  #[test]
  fn ascii_pattern_keeps_optimize_fast_path() {
    let reg = HybridRegex::new("foo").unwrap();
    assert!(matches!(reg, HybridRegex::Optimize(_)));
    assert!(reg.matches("foo"));

    let reg = HybridRegex::new(r"\w+").unwrap();
    assert!(matches!(reg, HybridRegex::Optimize(_)));
    assert!(reg.matches("abc"));
  }
}
