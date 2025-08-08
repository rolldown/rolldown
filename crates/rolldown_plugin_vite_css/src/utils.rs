use rolldown_utils::url::clean_url;

const CSS_LANGS: [&str; 9] =
  [".css", ".less", ".sass", ".scss", ".styl", ".stylus", ".pcss", ".postcss", ".sss"];

pub fn is_css_request(id: &str) -> bool {
  let cleaned_id = clean_url(id);
  CSS_LANGS.iter().any(|ext| cleaned_id.ends_with(ext))
}

#[inline]
pub fn is_css_module(id: &str) -> bool {
  memchr::memrchr(b'.', clean_url(id).as_bytes()).is_some_and(|i| id[..i].ends_with(".module"))
}
