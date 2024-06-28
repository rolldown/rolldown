use oxc::syntax::identifier;
use std::borrow::Cow;

pub fn is_validate_identifier_name(name: &str) -> bool {
  oxc::syntax::identifier::is_identifier_name(name)
}

pub fn legitimize_identifier_name(name: &str) -> Cow<str> {
  let mut legitimized = String::new();
  let mut have_seen_invalid_char = false;

  let mut chars_indices = name.char_indices();

  if let Some((_, first_char)) = chars_indices.next() {
    if identifier::is_identifier_start(first_char) {
      // Nothing we need to do
    } else {
      legitimized.push('_');
      have_seen_invalid_char = true;
    }
  }

  for (idx, char) in chars_indices {
    if identifier::is_identifier_part(char) {
      if have_seen_invalid_char {
        legitimized.push(char);
      }
    } else {
      if !have_seen_invalid_char {
        // See a invalid char for the first time
        have_seen_invalid_char = true;
        legitimized.push_str(&name[..idx]);
      }
      legitimized.push('_');
    }
  }

  if have_seen_invalid_char {
    Cow::Owned(legitimized)
  } else {
    Cow::Borrowed(name)
  }
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
