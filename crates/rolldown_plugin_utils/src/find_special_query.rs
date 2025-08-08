pub fn find_special_query(query: &str, param: &[u8]) -> Option<usize> {
  if query.len() < param.len() + 1 {
    return None;
  }
  if let Some(index) = memchr::memchr(b'?', query.as_bytes())
    && index + param.len() < query.len()
  {
    let bytes = &query.as_bytes()[index..];
    let len = bytes.len();
    let mut i = 1;
    while i < len {
      let p = i + param.len();
      if p <= len && &bytes[i..p] == param && (p == len || bytes[p] == b'&') {
        return Some(index + i);
      }
      while i < len && bytes[i] != b'&' {
        i += 1;
      }
      i += 1;
    }
  }
  None
}

#[test]
fn test_find_special_query() {
  let url = b"url";
  assert_eq!(find_special_query("?url", url), Some(1));
  assert_eq!(find_special_query("a?a=1&url", url), Some(6));
  assert_eq!(find_special_query("a?a=1&url&b=2", url), Some(6));
  assert_eq!(find_special_query("a?a=1&url=value", url), None);

  assert_eq!(find_special_query("a&url", url), None);
  assert_eq!(find_special_query("a&url&", url), None);
  assert_eq!(find_special_query("a&url=value", url), None);
  assert_eq!(find_special_query("url=value", url), None);
  assert_eq!(find_special_query("a?curl=123", url), None);
  assert_eq!(find_special_query("a?file=url.svg", url), None);
  assert_eq!(find_special_query("a?a=1&url=value", url), None);
}
