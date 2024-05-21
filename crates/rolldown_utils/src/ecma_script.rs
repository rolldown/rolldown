pub fn is_validate_identifier_name(name: &str) -> bool {
  oxc_syntax::identifier::is_identifier_name(name)
}

#[test]
fn test_is_validate_identifier_name() {
  assert!(is_validate_identifier_name("foo"));
  assert!(!is_validate_identifier_name("😈"));
}
