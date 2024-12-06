use std::borrow::Cow;

use oxc::syntax::keyword::RESERVED_KEYWORDS;

// Follow from https://github.com/rollup/rollup/blob/master/src/utils/identifierHelpers.ts#L17-L25
#[allow(clippy::needless_pass_by_value)]
pub fn sanitize_identifier(str: Cow<str>) -> String {
  if RESERVED_KEYWORDS.contains(str.as_ref()) {
    return format!("_{str}");
  }

  let mut sanitized = String::with_capacity(str.len());
  for char in str.chars() {
    // check start with number
    if sanitized.is_empty() && char.is_ascii_digit() {
      sanitized.push('_');
    }
    if char.is_ascii_alphanumeric() {
      sanitized.push(char);
    } else {
      sanitized.push('_');
    }
  }
  sanitized
}

#[test]
fn test_sanitize_identifier() {
  assert_eq!(sanitize_identifier("0a-b-c!".into()), "_0a_b_c_");
  assert_eq!(sanitize_identifier("class".into()), "_class");
}
