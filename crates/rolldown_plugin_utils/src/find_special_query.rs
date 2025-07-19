pub fn find_special_query(query: &str, param: &[u8]) -> Option<usize> {
  let param_len = param.len();
  if query.len() < param_len + 2 {
    return None;
  }
  for (index, _) in query.match_indices('?') {
    if index == 0 || index + param_len >= query.len() {
      return None;
    }
    let bytes = &query.as_bytes()[index..];
    let len = bytes.len();
    let mut i = 1;
    while i < len {
      let p = i + param_len;
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
  assert_eq!(find_special_query("a?a=1&url", url), Some(6));
  assert_eq!(find_special_query("a?a=1&url&b=2", url), Some(6));
  assert_eq!(find_special_query("a?a=1&url=value", url), None);

  assert_eq!(find_special_query("a&url", url), None);
  assert_eq!(find_special_query("a&url&", url), None);
  assert_eq!(find_special_query("a&url=value", url), None);
  assert_eq!(find_special_query("?url", url), None);
  assert_eq!(find_special_query("url=value", url), None);
  assert_eq!(find_special_query("a?curl=123", url), None);
  assert_eq!(find_special_query("a?file=url.svg", url), None);
  assert_eq!(find_special_query("a?a=1&url=value", url), None);
}
