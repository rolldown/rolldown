use std::borrow::Cow;

use super::find_special_query;

pub fn remove_special_query<'a>(url: &'a str, param: &[u8]) -> Cow<'a, str> {
  if let Some(start) = find_special_query(url, param) {
    let mut result = String::from(url);
    let mut end = start + param.len();
    if end != url.len() {
      end += 1;
    }
    result.replace_range(start..end, "");
    if result.ends_with(['?', '&']) {
      result.remove(result.len() - 1);
    }
    return Cow::Owned(result);
  }
  Cow::Borrowed(url)
}

#[test]
fn test_remove_special_query() {
  assert_eq!(remove_special_query("?url", b"url"), "");
  assert_eq!(remove_special_query("a?a=1&url", b"url"), "a?a=1");
  assert_eq!(remove_special_query("a?a=1&url&b=2", b"url"), "a?a=1&b=2");
  assert_eq!(remove_special_query("a?a=1&url=value", b"url"), "a?a=1&url=value");

  assert_eq!(remove_special_query("a&url", b"url"), "a&url");
  assert_eq!(remove_special_query("a&url&", b"url"), "a&url&");
  assert_eq!(remove_special_query("a&url=value", b"url"), "a&url=value");
  assert_eq!(remove_special_query("url=value", b"url"), "url=value");
  assert_eq!(remove_special_query("a?curl=123", b"url"), "a?curl=123");
  assert_eq!(remove_special_query("a?file=url.svg", b"url"), "a?file=url.svg");
  assert_eq!(remove_special_query("a?a=1&url=value", b"url"), "a?a=1&url=value");
}
