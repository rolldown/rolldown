use std::borrow::Cow;

use super::find_special_query;

pub fn remove_url_query(url: &str) -> Cow<'_, str> {
  if let Some(start) = find_special_query(url, b"url") {
    let mut result = String::from(url);
    let mut end = start + 3;
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
fn test_remove_url_query() {
  assert_eq!(remove_url_query("a?a=1&url"), "a?a=1");
  assert_eq!(remove_url_query("a?a=1&url&b=2"), "a?a=1&b=2");
  assert_eq!(remove_url_query("a?a=1&url=value"), "a?a=1&url=value");

  assert_eq!(remove_url_query("a&url"), "a&url");
  assert_eq!(remove_url_query("a&url&"), "a&url&");
  assert_eq!(remove_url_query("a&url=value"), "a&url=value");
  assert_eq!(remove_url_query("?url"), "?url");
  assert_eq!(remove_url_query("url=value"), "url=value");
  assert_eq!(remove_url_query("a?curl=123"), "a?curl=123");
  assert_eq!(remove_url_query("a?file=url.svg"), "a?file=url.svg");
  assert_eq!(remove_url_query("a?a=1&url=value"), "a?a=1&url=value");
}
