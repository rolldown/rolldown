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
  // Match pattern: /\.module\.(css|less|sass|scss|styl|stylus|pcss|postcss|sss)(?:$|\?)/
  CSS_LANGS.iter().any(|ext| {
    id.rfind(ext).is_some_and(|pos| {
      let after = pos + ext.len();
      let after_ok = after == id.len() || id.as_bytes().get(after) == Some(&b'?');
      let before_ok = id[..pos].ends_with(".module");
      after_ok && before_ok
    })
  })
}
