/// Find the end position of a shebang in the content.
///
/// Returns a tuple of (end_position, has_shebang).
/// - If a shebang exists, returns the position after the newline and `true`.
/// - If no shebang exists, returns `(0, false)`.
pub fn find_shebang_end(content: &str) -> (usize, bool) {
  if !content.starts_with("#!") {
    return (0, false);
  }

  match memchr::memchr(b'\n', content.as_bytes()) {
    Some(pos) => (pos + 1, true),
    None => (content.len(), true),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_find_shebang_end_with_lf() {
    let content = "#!/usr/bin/env node\nconsole.log('hello');";
    let (end, has_shebang) = find_shebang_end(content);
    assert!(has_shebang);
    assert_eq!(end, 20); // Position after the newline
    assert_eq!(&content[..end], "#!/usr/bin/env node\n");
    assert_eq!(&content[end..], "console.log('hello');");
  }

  #[test]
  fn test_find_shebang_end_with_crlf() {
    let content = "#!/usr/bin/env node\r\nconsole.log('hello');";
    let (end, has_shebang) = find_shebang_end(content);
    assert!(has_shebang);
    assert_eq!(end, 21); // Position after \r\n (the \n is at position 20, so end is 21)
    assert_eq!(&content[..end], "#!/usr/bin/env node\r\n");
    assert_eq!(&content[end..], "console.log('hello');");
  }

  #[test]
  fn test_find_shebang_end_no_shebang() {
    let content = "console.log('hello');";
    let (end, has_shebang) = find_shebang_end(content);
    assert!(!has_shebang);
    assert_eq!(end, 0);
  }

  #[test]
  fn test_find_shebang_end_no_newline() {
    let content = "#!/usr/bin/env node";
    let (end, has_shebang) = find_shebang_end(content);
    assert!(has_shebang);
    assert_eq!(end, content.len());
    assert_eq!(&content[..end], "#!/usr/bin/env node");
  }
}
