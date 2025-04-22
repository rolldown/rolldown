macro_rules! matches_invalid_chars {
  ($chars:ident) => {
    matches!($chars,
      '\u{0000}'
        ..='\u{001f}'
          | '"'
          | '#'
          | '$'
          | '%'
          | '&'
          | '*'
          | '+'
          | ','
          | ':'
          | ';'
          | '<'
          | '='
          | '>'
          | '?'
          | '['
          | ']'
          | '^'
          | '`'
          | '{'
          | '|'
          | '}'
          | '\u{007f}'
    )
  };
}

// Follow from https://github.com/rollup/rollup/blob/master/src/utils/sanitizeFileName.ts
pub fn default_sanitize_file_name(str: &str) -> String {
  let mut sanitized = String::with_capacity(str.len());
  let mut chars = str.chars();

  // A `:` is only allowed as part of a windows drive letter (ex: C:\foo)
  // Otherwise, avoid them because they can refer to NTFS alternate data streams.
  if starts_with_windows_drive(str) {
    sanitized.push(chars.next().unwrap());
    sanitized.push(chars.next().unwrap());
  }

  for char in chars {
    if matches_invalid_chars!(char) {
      sanitized.push('_');
    } else {
      sanitized.push(char);
    }
  }
  sanitized
}

fn starts_with_windows_drive(str: &str) -> bool {
  let mut chars = str.chars();
  if !chars.next().is_some_and(|c| c.is_ascii_alphabetic()) {
    return false;
  }
  chars.next().is_some_and(|c| c == ':')
}

#[test]
fn test_sanitize_file_name() {
  assert_eq!(default_sanitize_file_name("\0+a=Z_0-"), "__a_Z_0-");
}
