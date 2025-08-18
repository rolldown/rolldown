use rolldown_utils::url::clean_url;

use super::find_special_query;

const CSS_LANGS: [&str; 9] =
  [".css", ".less", ".sass", ".scss", ".styl", ".stylus", ".pcss", ".postcss", ".sss"];

const SPECIAL_QUERY: [&str; 5] = ["commonjs-proxy", "worker", "sharedworker", "raw", "url"];

#[inline]
pub fn is_css_request(id: &str) -> bool {
  let cleaned_id = clean_url(id);
  CSS_LANGS.iter().any(|ext| cleaned_id.ends_with(ext))
}

#[inline]
pub fn is_special_query(id: &str) -> bool {
  SPECIAL_QUERY.iter().any(|query| find_special_query(id, query.as_bytes()).is_some())
}
