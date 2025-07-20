use std::{borrow::Cow, path::Path};

use cow_utils::CowUtils as _;
use rolldown_plugin_utils::find_special_query;
use rolldown_utils::{
  pattern_filter::{StringOrRegex, filter as pattern_filter},
  url::clean_url,
};

pub const KNOWN_ASSET_TYPES: [&str; 34] = [
  // images
  "apng",
  "bmp",
  "png",
  "jpg",
  "jpeg",
  "jfif",
  "pjpeg",
  "pjp",
  "gif",
  "svg",
  "ico",
  "webp",
  "avif",
  "cur",
  "jxl",
  // media
  "mp4",
  "webm",
  "ogg",
  "mp3",
  "wav",
  "flac",
  "aac",
  "opus",
  "mov",
  "m4a",
  "vtt",
  // fonts
  "woff",
  "woff2",
  "eot",
  "ttf",
  "otf",
  // other
  "webmanifest",
  "pdf",
  "txt",
];

impl super::AssetPlugin {
  pub fn is_assets_include(&self, cwd: &Path, cleaned_id: &str) -> bool {
    if let Some(ext) = Path::new(cleaned_id).extension().and_then(|e| e.to_str()) {
      if KNOWN_ASSET_TYPES.contains(&ext.cow_to_ascii_lowercase().as_ref()) {
        return true;
      }
    }
    if self.assets_include.is_empty() {
      return false;
    }
    pattern_filter(
      None::<&[StringOrRegex]>,
      Some(&self.assets_include),
      cleaned_id,
      cwd.to_string_lossy().as_ref(),
    )
    .inner()
  }

  pub fn is_not_valid_assets(&self, cwd: &Path, id: &str) -> bool {
    if find_special_query(id, b"url").is_some() {
      return false;
    }
    !self.is_assets_include(cwd, clean_url(id))
  }
}

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
