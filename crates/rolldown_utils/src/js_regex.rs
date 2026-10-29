use std::{borrow::Cow, fmt, ops::Range};

use crate::concat_string;

/// Uses `regex` for common JavaScript patterns and falls back to `regress` for
/// features such as backreferences and lookaround assertions.
#[derive(Clone)]
pub struct HybridRegex(HybridRegexInner);

#[derive(Clone)]
enum HybridRegexInner {
  Optimized(regex::Regex),
  Ecma(regress::Regex),
}

impl fmt::Debug for HybridRegex {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match &self.0 {
      HybridRegexInner::Optimized(regex) => {
        f.debug_tuple("Optimized").field(&regex.as_str()).finish()
      }
      // `regress::Regex`'s derived formatter walks its compiled instruction program. Apart from
      // producing noisy output, calling it retains every instruction's formatter in release builds.
      HybridRegexInner::Ecma(_) => f.write_str("Ecma(..)"),
    }
  }
}

/// An iterator over matches from the ECMAScript fallback engine.
#[derive(Debug)]
pub struct HybridMatches<'regex, 'text>(regress::Matches<'regex, 'text>);

/// A match from the ECMAScript fallback engine.
#[derive(Debug, Clone)]
pub struct HybridMatch(regress::Match);

impl Iterator for HybridMatches<'_, '_> {
  type Item = HybridMatch;

  #[inline(never)]
  fn next(&mut self) -> Option<Self::Item> {
    self.0.next().map(HybridMatch)
  }
}

impl HybridMatch {
  pub fn range(&self) -> &Range<usize> {
    &self.0.range
  }

  pub fn group_count(&self) -> usize {
    1 + self.0.captures.len()
  }

  pub fn group(&self, index: usize) -> Option<Range<usize>> {
    self.0.group(index)
  }
}

// Convenience conversion for tests.
impl From<&str> for HybridRegex {
  fn from(pattern: &str) -> Self {
    HybridRegex::new(pattern).unwrap_or_else(|err| {
      panic!("failed to create HybridRegex from {pattern}, error details: {err}")
    })
  }
}
impl HybridRegex {
  #[inline(never)]
  pub fn new(pattern: &str) -> anyhow::Result<Self> {
    let regex_pattern = Self::get_regex_pattern(pattern, "");
    match regex::Regex::new(&regex_pattern).map(Self::from_optimized) {
      Ok(reg) => Ok(reg),
      Err(_) => Self::new_ecma(pattern),
    }
  }

  pub fn from_optimized(regex: regex::Regex) -> Self {
    Self(HybridRegexInner::Optimized(regex))
  }

  #[inline(never)]
  pub fn new_ecma(pattern: &str) -> anyhow::Result<Self> {
    regress::Regex::new(pattern).map(HybridRegexInner::Ecma).map(Self).map_err(anyhow::Error::from)
  }

  #[inline(never)]
  pub fn with_flags(pattern: &str, flags: &str) -> anyhow::Result<Self> {
    let regex_pattern = Self::get_regex_pattern(pattern, flags);
    match regex::Regex::new(&regex_pattern).map(Self::from_optimized) {
      Ok(reg) => Ok(reg),
      Err(_) => regress::Regex::with_flags(pattern, flags)
        .map(HybridRegexInner::Ecma)
        .map(Self)
        .map_err(anyhow::Error::from),
    }
  }

  pub fn as_optimized(&self) -> Option<&regex::Regex> {
    match &self.0 {
      HybridRegexInner::Optimized(regex) => Some(regex),
      HybridRegexInner::Ecma(_) => None,
    }
  }

  /// Returns matches from the ECMAScript fallback, or `None` when this regex
  /// uses the optimized engine.
  // Keep every call into regress behind an out-of-line boundary. Its public
  // matching entry points are inline and would otherwise copy the executor
  // into each crate using HybridRegex.
  #[inline(never)]
  pub fn find_from<'regex, 'text>(
    &'regex self,
    text: &'text str,
    start: usize,
  ) -> Option<HybridMatches<'regex, 'text>> {
    match &self.0 {
      HybridRegexInner::Optimized(_) => None,
      HybridRegexInner::Ecma(regex) => Some(HybridMatches(regex.find_from(text, start))),
    }
  }

  pub fn regex_pattern(&self) -> Option<&str> {
    match &self.0 {
      HybridRegexInner::Optimized(regex) => Some(regex.as_str()),
      HybridRegexInner::Ecma(_) => None,
    }
  }

  fn get_regex_pattern(pattern: &str, flags: &str) -> String {
    // ECMAScript regex treats CRLF as a line break like LF
    // (e.g. `/a$/m.test("a\r\n")` and `/a$/m.test("a\n")` both returns `true`)
    // Also when `s` flag is not used, `.` does not match CRLF like LF does not.
    concat_string!("(?R", flags, ")", pattern)
  }

  pub fn matches(&self, text: &str) -> bool {
    match &self.0 {
      HybridRegexInner::Optimized(regex) => regex.is_match(text),
      HybridRegexInner::Ecma(_) => self.matches_ecma(text),
    }
  }

  pub fn replace<'a>(&self, haystack: &'a str, replacement: &str) -> Cow<'a, str> {
    match &self.0 {
      HybridRegexInner::Optimized(regex) => regex.replace(haystack, replacement),
      HybridRegexInner::Ecma(_) => {
        // `regress` uses regex-crate-style replacement tokens, not full
        // `String.prototype.replace` semantics. Numbered captures match JS and
        // are what Vite aliases rely on.
        self.replace_ecma(haystack, replacement)
      }
    }
  }

  pub fn replace_all<'a>(&self, haystack: &'a str, replacement: &str) -> Cow<'a, str> {
    match &self.0 {
      HybridRegexInner::Optimized(regex) => regex.replace_all(haystack, replacement),
      HybridRegexInner::Ecma(_) => self.replace_all_ecma(haystack, replacement),
    }
  }

  #[inline(never)]
  fn matches_ecma(&self, text: &str) -> bool {
    self.find_from(text, 0).is_some_and(|mut matches| matches.next().is_some())
  }

  #[inline(never)]
  fn replace_ecma<'a>(&self, haystack: &'a str, replacement: &str) -> Cow<'a, str> {
    let HybridRegexInner::Ecma(regex) = &self.0 else { unreachable!() };
    let Some(mut matches) = self.find_from(haystack, 0) else { unreachable!() };
    if matches.next().is_none() {
      return Cow::Borrowed(haystack);
    }
    Cow::Owned(regex.replace(haystack, replacement))
  }

  #[inline(never)]
  fn replace_all_ecma<'a>(&self, haystack: &'a str, replacement: &str) -> Cow<'a, str> {
    let HybridRegexInner::Ecma(regex) = &self.0 else { unreachable!() };
    let Some(mut matches) = self.find_from(haystack, 0) else { unreachable!() };
    if matches.next().is_none() {
      return Cow::Borrowed(haystack);
    }
    Cow::Owned(regex.replace_all(haystack, replacement))
  }

  #[cfg(test)]
  fn uses_ecma(&self) -> bool {
    matches!(self.0, HybridRegexInner::Ecma(_))
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
    assert!(reg.uses_ecma());
    assert_eq!(reg.replace_all("111aa111", "1"), "1aa1");
  }

  #[test]
  fn ecma_replace_expands_replacement_tokens() {
    let reg = HybridRegex::new(r"^@app(?!/(?:excluded))(/.*)?$").unwrap();
    assert!(reg.uses_ecma());
    assert_eq!(reg.replace("@app/utils", "/abs/src/app$1"), "/abs/src/app/utils");

    let reg = HybridRegex::new(r"(foo)(?!bar)").unwrap();
    assert!(reg.uses_ecma());
    assert_eq!(reg.replace("foo baz", "$0:$$:$1:$2"), "foo:$:foo: baz");
  }

  #[test]
  fn ecma_replace_all_expands_replacement_tokens() {
    let reg = HybridRegex::new(r"(\d+)(?=px)").unwrap();
    assert!(reg.uses_ecma());
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
