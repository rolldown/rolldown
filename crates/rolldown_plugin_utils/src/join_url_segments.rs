use std::borrow::Cow;

pub fn join_url_segments<'a>(mut a: &'a str, b: &'a str) -> Cow<'a, str> {
  if a.is_empty() || b.is_empty() {
    return Cow::Borrowed(if a.is_empty() { b } else { a });
  }
  if a.ends_with('/') {
    a = &a[..a.len() - 1];
  }
  Cow::Owned(format!("{a}{}{b}", if b.starts_with('/') { "" } else { "/" }))
}
