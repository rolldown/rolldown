use std::borrow::Cow;

// TODO support `output.sanitizeFileName`
// Follow from https://github.com/rollup/rollup/blob/master/src/utils/sanitizeFileName.ts
#[allow(clippy::needless_pass_by_value)]
pub fn sanitize_file_name(str: Cow<str>) -> String {
  let mut sanitized = String::with_capacity(str.len());
  for char in str.chars() {
    if char.is_ascii_alphanumeric() || matches!(char, '-' | '_') {
      sanitized.push(char);
    } else {
      sanitized.push('_');
    }
  }
  sanitized
}

#[test]
fn test_sanitize_file_name() {
  assert_eq!(sanitize_file_name("\0+a=Z_0-".into()), "__a_Z_0-");
}
