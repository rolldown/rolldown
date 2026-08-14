use std::borrow::Cow;

use crate::concat_string;

/// According to the doc of `regress`, https://docs.rs/regress/0.10.0/regress/#comparison-to-regex-crate
/// **regress supports features that regex does not, in particular backreferences and zero-width lookaround assertions.**
/// these features are not commonly used, so in most cases the slow path will not be reached.
#[derive(Debug, Clone)]
pub enum HybridRegex {
  Optimize(regex::Regex),
  Ecma(regress::Regex),
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
    let regex_pattern = Self::get_regex_pattern(pattern, "");
    match regex::Regex::new(&regex_pattern).map(HybridRegex::Optimize) {
      Ok(reg) => Ok(reg),
      Err(_) => regress::Regex::new(pattern).map(HybridRegex::Ecma).map_err(anyhow::Error::from),
    }
  }

  pub fn with_flags(pattern: &str, flags: &str) -> anyhow::Result<Self> {
    let regex_pattern = Self::get_regex_pattern(pattern, flags);
    match regex::Regex::new(&regex_pattern).map(HybridRegex::Optimize) {
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
}
