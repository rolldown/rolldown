use rolldown_utils::url::clean_url;

const CSS_LANGS: [&str; 9] =
  [".css", ".less", ".sass", ".scss", ".styl", ".stylus", ".pcss", ".postcss", ".sss"];

#[inline]
pub fn is_css_request(id: &str) -> bool {
  // Match pattern: /\.(css|less|sass|scss|styl|stylus|pcss|postcss|sss)(?:$|\?)/
  CSS_LANGS.iter().any(|ext| {
    id.rfind(ext).is_some_and(|pos| {
      let after = pos + ext.len();
      after == id.len() || id.as_bytes().get(after) == Some(&b'?')
    })
  })
}

#[inline]
pub fn is_css_module(id: &str) -> bool {
  memchr::memrchr(b'.', clean_url(id).as_bytes()).is_some_and(|i| id[..i].ends_with(".module"))
}
