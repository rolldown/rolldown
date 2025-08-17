use rolldown_utils::url::clean_url;

#[inline]
pub fn is_css_module(id: &str) -> bool {
  memchr::memrchr(b'.', clean_url(id).as_bytes()).is_some_and(|i| id[..i].ends_with(".module"))
}
