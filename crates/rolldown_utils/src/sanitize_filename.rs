use std::borrow::Cow;

// Follow from https://github.com/rollup/rollup/blob/master/src/utils/sanitizeFileName.ts
//
// Every character that must be replaced is ASCII (<= 0x7F). UTF-8 guarantees that no byte of a
// multi-byte character is < 0x80, so scanning bytes finds exactly the same positions as scanning
// chars — without decoding — and every match lands on a char boundary.
const fn is_invalid_byte(byte: u8) -> bool {
  matches!(
    byte,
    0x00
      ..=0x1f // C0 control characters
      | b'"'
      | b'#'
      | b'$'
      | b'%'
      | b'&'
      | b'*'
      | b'+'
      | b','
      | b':'
      | b';'
      | b'<'
      | b'='
      | b'>'
      | b'?'
      | b'['
      | b']'
      | b'^'
      | b'`'
      | b'{'
      | b'|'
      | b'}'
      | 0x7f // DEL
  )
}

pub fn default_sanitize_file_name(str: &str) -> Cow<'_, str> {
  // A `:` is only allowed as part of a windows drive letter (ex: C:\foo).
  // Otherwise, avoid them because they can refer to NTFS alternate data streams.
  // Skip the two-byte drive prefix so its `:` is preserved, but every later `:`
  // is still treated as invalid. Both prefix bytes are ASCII, so `2` is a char boundary.
  let scan_start = if starts_with_windows_drive(str) { 2 } else { 0 };

  let Some(first_invalid) = str.as_bytes()[scan_start..]
    .iter()
    .position(|&byte| is_invalid_byte(byte))
    .map(|idx| scan_start + idx)
  else {
    // Common case: nothing needs sanitizing, return the input untouched (no allocation).
    return Cow::Borrowed(str);
  };

  // Allocate only on the path that actually rewrites the name. The valid prefix
  // (drive letter included) is copied in bulk instead of one char at a time.
  let mut sanitized = String::with_capacity(str.len());
  sanitized.push_str(&str[..first_invalid]);
  for char in str[first_invalid..].chars() {
    // Invalid chars are all ASCII; `u8::try_from` keeps non-ASCII chars (and any char above
    // U+00FF) on the copy path instead of truncating them into a false match (e.g. `😀 as u8`).
    match u8::try_from(char) {
      Ok(byte) if is_invalid_byte(byte) => sanitized.push('_'),
      _ => sanitized.push(char),
    }
  }
  Cow::Owned(sanitized)
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

  // Clean inputs are returned untouched and borrowed (no allocation).
  assert_eq!(default_sanitize_file_name("index.js"), "index.js");
  assert!(matches!(default_sanitize_file_name("a/b/c.js"), Cow::Borrowed("a/b/c.js")));
  assert!(matches!(default_sanitize_file_name(""), Cow::Borrowed("")));

  // A drive-letter `:` is preserved, but any later `:` is replaced.
  assert!(matches!(default_sanitize_file_name("C:/foo.js"), Cow::Borrowed("C:/foo.js")));
  assert_eq!(default_sanitize_file_name("C:/a:b.js"), "C:/a_b.js");

  // Dirty inputs are rewritten and owned.
  assert!(matches!(default_sanitize_file_name("a?b"), Cow::Owned(_)));
}

#[test]
fn test_sanitize_unicode() {
  // Clean multi-byte names contain no invalid bytes, so they are returned untouched and
  // borrowed — the byte scan must never mistake a UTF-8 byte (>= 0x80) for an ASCII invalid
  // char. Covers 2-, 3-, and 4-byte sequences.
  assert!(matches!(default_sanitize_file_name("café.js"), Cow::Borrowed("café.js")));
  assert!(matches!(default_sanitize_file_name("日本語.js"), Cow::Borrowed("日本語.js")));
  assert!(matches!(
    default_sanitize_file_name("компоненты/Кнопка.js"),
    Cow::Borrowed("компоненты/Кнопка.js")
  ));
  assert!(matches!(default_sanitize_file_name("emoji_😀.js"), Cow::Borrowed("emoji_😀.js")));

  // On the rewrite path, multi-byte chars must survive verbatim and never be truncated into a
  // false match: `é as u8 == 0xe9` (in range, not invalid) and `😀 as u8 == 0x00` (would
  // otherwise hit the control-char range). Only the ASCII `?` / `:` are replaced.
  assert_eq!(default_sanitize_file_name("a?é"), "a_é");
  assert_eq!(default_sanitize_file_name("a?😀"), "a_😀");
  assert_eq!(default_sanitize_file_name("日本?語"), "日本_語");
  assert_eq!(default_sanitize_file_name("café:dir.js"), "café_dir.js");
}
