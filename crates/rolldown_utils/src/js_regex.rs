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

impl HybridRegex {
  pub fn new(pattern: &str) -> anyhow::Result<Self> {
    match regex::Regex::new(pattern).map(HybridRegex::Optimize) {
      Ok(reg) => Ok(reg),
      Err(_) => regress::Regex::new(pattern).map(HybridRegex::Ecma).map_err(anyhow::Error::from),
    }
  }

  pub fn with_flags(pattern: &str, flags: &str) -> anyhow::Result<Self> {
    let regex_pattern =
      if flags.is_empty() { pattern } else { &concat_string!("(?", flags, ")", pattern) };

    match regex::Regex::new(regex_pattern).map(HybridRegex::Optimize) {
      Ok(reg) => Ok(reg),
      Err(_) => regress::Regex::with_flags(pattern, flags)
        .map(HybridRegex::Ecma)
        .map_err(anyhow::Error::from),
    }
  }

  pub fn matches(&self, text: &str) -> bool {
    match self {
      HybridRegex::Optimize(reg) => reg.is_match(text),
      HybridRegex::Ecma(reg) => reg.find(text).is_some(),
    }
  }

  pub fn replace_all<'a>(&self, haystack: &'a str, replacement: &str) -> Cow<'a, str> {
    match self {
      HybridRegex::Optimize(r) => r.replace_all(haystack, replacement),
      HybridRegex::Ecma(reg) => regress_regexp_replace_all(reg, haystack, replacement),
    }
  }
}

fn regress_regexp_replace_all<'h>(
  reg: &regress::Regex,
  haystack: &'h str,
  replacement: &str,
) -> Cow<'h, str> {
  let iter = reg.find_iter(haystack);
  let mut iter = iter.peekable();
  if iter.peek().is_none() {
    return Cow::Borrowed(haystack);
  }

  let mut ret = String::with_capacity(haystack.len());
  let mut last = 0;
  for m in iter {
    ret.push_str(&haystack[last..m.start()]);
    ret.push_str(replacement);
    last = m.end();
  }
  ret.push_str(&haystack[last..]);
  Cow::Owned(ret)
}

#[cfg(test)]
mod test {
  use crate::js_regex::regress_regexp_replace_all;

  #[test]
  fn with_flags() {
    let reg = super::HybridRegex::with_flags("a", "i").unwrap();
    assert!(reg.matches("A"));

    let reg = super::HybridRegex::new("a").unwrap();
    assert!(!reg.matches("A"));
  }

  #[test]
  fn regress_replace_all() {
    let reg = regress::Regex::new("\\d+").unwrap();
    assert_eq!(regress_regexp_replace_all(&reg, "111aa111", "1"), "1aa1");
  }
}
