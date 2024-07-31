use oxc::syntax::{identifier, keyword};
use std::borrow::Cow;

pub fn is_validate_identifier_name(name: &str) -> bool {
  identifier::is_identifier_name(name)
}

pub fn is_validate_assignee_identifier_name(name: &str) -> bool {
  // Because of the mistake in oxc (related: oxc-project/oxc#4484, we need to use separated functions in `keyword`.
  // TODO use `is_reserved_keyword_or_global_object` after the fix
  identifier::is_identifier_name(name)
    && !keyword::is_reserved_keyword(name)
    && !keyword::is_global_object(name)
}

pub fn legitimize_identifier_name(name: &str) -> Cow<str> {
  let mut legitimized = String::new();
  let mut chars_indices = name.char_indices();

  let mut first_invalid_char_index = None;

  if let Some((idx, first_char)) = chars_indices.next() {
    if identifier::is_identifier_start(first_char) {
      // Nothing we need to do
    } else {
      first_invalid_char_index = Some(idx);
    }
  }

  if first_invalid_char_index.is_none() {
    first_invalid_char_index =
      chars_indices.find(|(_idx, char)| !identifier::is_identifier_part(*char)).map(|(idx, _)| idx);
  }

  if let Some(first_invalid_char_index) = first_invalid_char_index {
    let (first_valid_part, rest_part) = name.split_at(first_invalid_char_index);
    legitimized.push_str(first_valid_part);
    for char in rest_part.chars() {
      if identifier::is_identifier_part(char) {
        legitimized.push(char);
      } else {
        legitimized.push('_');
      }
    }

    return Cow::Owned(legitimized);
  }

  Cow::Borrowed(name)
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
