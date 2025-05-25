use std::{borrow::Cow, path::Path};

use cow_utils::CowUtils as _;
use rolldown_plugin_utils::find_special_query;
use rolldown_utils::{
  pattern_filter::{StringOrRegex, filter as pattern_filter},
  url::clean_url,
};
use sugar_path::SugarPath as _;

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
    cleaned_id.as_path().extension().is_some_and(|ext| {
      let ext = ext.to_string_lossy();
      let ext = ext.cow_to_ascii_lowercase();
      KNOWN_ASSET_TYPES.contains(&ext.as_ref())
    }) || (!self.assets_include.is_empty()
      && pattern_filter(
        None::<&[StringOrRegex]>,
        Some(&self.assets_include),
        cleaned_id,
        cwd.to_string_lossy().as_ref(),
      )
      .inner())
  }

  pub fn is_not_valid_assets(&self, cwd: &Path, id: &str) -> bool {
    let cleaned_id = clean_url(id);
    (cleaned_id.len() == id.len() || find_special_query(id, b"url").is_none())
      && !self.is_assets_include(cwd, cleaned_id)
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
