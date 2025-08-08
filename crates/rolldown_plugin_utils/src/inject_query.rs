use rolldown_utils::{pattern_filter::normalize_path, url::clean_url};

pub fn inject_query(url: &str, query: &str) -> String {
  let mut out = String::with_capacity(url.len() + query.len() + 1);
  let normalized = normalize_path(clean_url(url));

  out.push_str(&normalized);
  out.push('?');
  out.push_str(query);

  if url.len() != normalized.len() {
    let postfix = &url[normalized.len()..];
    if let Some(postfix) = postfix.strip_prefix('?') {
      out.push('&');
      out.push_str(postfix);
    } else {
      out.push_str(postfix);
    }
  }

  out
}

#[test]
fn test() {
  assert_eq!(inject_query("a/b", "url"), "a/b?url");
  assert_eq!(inject_query("a/b?c", "url"), "a/b?url&c");
  assert_eq!(inject_query("a/b?c&d=e", "url"), "a/b?url&c&d=e");
}
