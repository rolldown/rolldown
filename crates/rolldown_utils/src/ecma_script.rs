use regex::Regex;
use std::borrow::Cow;

pub fn is_validate_identifier_name(name: &str) -> bool {
  oxc_syntax::identifier::is_identifier_name(name)
}

pub fn legitimize_identifier_name(name: &str) -> Cow<str> {
  static VALID_RE: once_cell::sync::Lazy<Regex> =
    once_cell::sync::Lazy::new(|| Regex::new(r"[^a-zA-Z0-9_$]").unwrap());

  VALID_RE.replace_all(name, "_")
}

#[test]
fn test_is_validate_identifier_name() {
  assert!(is_validate_identifier_name("foo"));
  assert!(!is_validate_identifier_name("😈"));
}

#[test]
fn test_legitimize_identifier_name() {
  assert_eq!(legitimize_identifier_name("foo"), "foo");
  assert_eq!(legitimize_identifier_name("$foo$"), "$foo$");
  assert_eq!(legitimize_identifier_name("react-dom"), "react_dom");
}
