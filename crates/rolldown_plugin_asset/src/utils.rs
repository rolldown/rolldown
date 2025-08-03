use std::path::Path;

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

pub enum InvalidAsset {
  True,
  False,
  Special,
}

impl InvalidAsset {
  pub fn is(&self) -> bool {
    matches!(self, Self::True)
  }
}

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

  pub fn check_invalid_assets(&self, cwd: &Path, id: &str) -> InvalidAsset {
    if find_special_query(id, b"url").is_some() {
      InvalidAsset::False
    } else if self.is_assets_include(cwd, clean_url(id)) {
      InvalidAsset::Special
    } else {
      InvalidAsset::True
    }
  }
}
