use memchr::memmem;

#[allow(clippy::cast_possible_truncation)]
#[inline]
pub fn lines_count(str: &str) -> u32 {
  memmem::find_iter(str.as_bytes(), "\n").count() as u32
}

#[test]
fn test() {
  assert_eq!(lines_count("a\nb\nc"), 2);
  assert_eq!(lines_count("a\nb\nc\n"), 3);
  assert_eq!(lines_count("a"), 0);
}
