use std::path::{Path, PathBuf};
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

#[inline]
pub fn has_special_ext(path: impl AsRef<Path>) -> bool {
  let extension = path.as_ref().extension();
  extension.is_some_and(|ext| KNOWN_ASSET_TYPES.contains(&ext.to_string_lossy().as_ref()))
}

#[inline]
pub fn contains_url_param(query: &str) -> bool {
  if query.len() < 5 {
    return false;
  }
  for (index, _) in query.match_indices('?') {
    if index == 0 || index + 4 > query.len() {
      return false;
    }
    let bytes = &query.as_bytes()[index..];
    let len = bytes.len();
    let mut i = 1;
    while i < len {
      if i + 3 <= len
        && &bytes[i..i + 3] == b"url"
        && (i + 3 == len || bytes[i + 3] == b'=' || bytes[i + 3] == b'&')
      {
        return true;
      }
      while i < len && bytes[i] != b'&' {
        i += 1;
      }
      i += 1;
    }
  }
  false
}

pub fn check_public_file(id: &str, public_dir: Option<&str>) -> Option<PathBuf> {
  if id.is_empty() || id.as_bytes()[0] != b'/' {
    return None;
  }
  if let Some(dir) = public_dir {
    let file = Path::new(dir).join(&id[1..]).normalize();
    if file.starts_with(dir) && file.exists() {
      return Some(file);
    }
  }
  None
}

#[test]
fn test_contains_url_param() {
  assert!(contains_url_param("a?a=1&url"));
  assert!(contains_url_param("a?a=1&url&b=2"));
  assert!(contains_url_param("a?a=1&url=value"));

  assert!(!contains_url_param("a&url"));
  assert!(!contains_url_param("a&url&"));
  assert!(!contains_url_param("a&url=value"));
  assert!(!contains_url_param("?url"));
  assert!(!contains_url_param("url=value"));
  assert!(!contains_url_param("a?curl=123"));
  assert!(!contains_url_param("a?file=url.svg"));
}
