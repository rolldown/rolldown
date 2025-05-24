use std::{borrow::Cow, path::Path};

use cow_utils::CowUtils as _;
use rolldown_plugin_utils::check_public_file;
use rolldown_utils::{
  dataurl::encode_as_shortest_dataurl,
  mime::guess_mime,
  pattern_filter::{StringOrRegex, filter as pattern_filter, normalize_path},
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
  pub fn is_not_valid_assets(&self, cwd: &Path, id: &str) -> bool {
    let cleaned_id = clean_url(id);
    let is_valid_assets = has_special_ext(cleaned_id)
      || ((cleaned_id.len() != id.len()) && find_query_param(id, b"url").is_some());

    !is_valid_assets
      && (self.assets_include.is_empty()
        || !pattern_filter(
          None::<&[StringOrRegex]>,
          Some(&self.assets_include),
          cleaned_id,
          cwd.to_string_lossy().as_ref(),
        )
        .inner())
  }

  pub fn file_to_dev_url(&self, id: &str, root: &Path) -> anyhow::Result<String> {
    let cleaned_id = clean_url(id);
    let public_file = check_public_file(cleaned_id, self.public_dir.as_deref())
      .map(|file| Cow::Owned(file.to_string_lossy().into_owned()));

    if find_query_param(id, b"inline").is_some() {
      let file = public_file.unwrap_or(Cow::Borrowed(cleaned_id));
      let content = std::fs::read_to_string(&*file)?;
      return asset_to_data_url(file.as_path(), content.as_bytes());
    }

    // TODO(shulaoda): align below logic
    // if cleaned_id.ends_with(".svg") {}

    let id = normalize_path(id);
    let url = if public_file.is_some() {
      Cow::Borrowed(id.strip_suffix("/").unwrap_or(&id))
    } else {
      let path = Path::new(id.as_ref());
      Cow::Owned(if path.starts_with(root) {
        format!("/{}", path.relative(root).to_slash_lossy())
      } else {
        format!("/@fs/{id}")
      })
    };

    Ok(if self.url_base.is_empty() {
      url.into_owned()
    } else {
      let base = self.url_base.strip_suffix('/').unwrap_or(&self.url_base);
      let url = url.strip_prefix('/').unwrap_or(&url);
      format!("{base}/{url}")
    })
  }

  pub fn file_to_built_url(&self, _id: &str) -> anyhow::Result<String> {
    todo!()
  }
}

fn asset_to_data_url(path: &Path, content: &[u8]) -> anyhow::Result<String> {
  // TODO(shulaoda): should give an warning
  // if (environment.config.build.lib && isGitLfsPlaceholder(content)) {
  //   environment.logger.warn(
  //     colors.yellow(`Inlined file ${file} was not downloaded via Git LFS`),
  //   )
  // }
  let guessed_mime = guess_mime(path, content)?;
  Ok(encode_as_shortest_dataurl(&guessed_mime, content))
}

#[inline]
pub fn has_special_ext(path: impl AsRef<Path>) -> bool {
  let extension = path.as_ref().extension();
  extension.is_some_and(|ext| {
    let ext = ext.to_string_lossy();
    let ext = ext.cow_to_ascii_lowercase();
    KNOWN_ASSET_TYPES.contains(&ext.as_ref())
  })
}

#[inline]
pub fn find_query_param(query: &str, param: &[u8]) -> Option<usize> {
  let param_len = param.len();
  if query.len() < param_len + 2 {
    return None;
  }
  for (index, _) in query.match_indices('?') {
    if index == 0 || index + param_len >= query.len() {
      return None;
    }
    let bytes = &query.as_bytes()[index..];
    let len = bytes.len();
    let mut i = 1;
    while i < len {
      let p = i + param_len;
      if p <= len && &bytes[i..p] == param && (p == len || bytes[p] == b'&') {
        return Some(index + i);
      }
      while i < len && bytes[i] != b'&' {
        i += 1;
      }
      i += 1;
    }
  }
  None
}

pub fn remove_url_query(url: &str) -> Cow<'_, str> {
  if let Some(start) = find_query_param(url, b"url") {
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
fn test_find_query_param() {
  let url = b"url";
  assert_eq!(find_query_param("a?a=1&url", url), Some(6));
  assert_eq!(find_query_param("a?a=1&url&b=2", url), Some(6));
  assert_eq!(find_query_param("a?a=1&url=value", url), None);

  assert_eq!(find_query_param("a&url", url), None);
  assert_eq!(find_query_param("a&url&", url), None);
  assert_eq!(find_query_param("a&url=value", url), None);
  assert_eq!(find_query_param("?url", url), None);
  assert_eq!(find_query_param("url=value", url), None);
  assert_eq!(find_query_param("a?curl=123", url), None);
  assert_eq!(find_query_param("a?file=url.svg", url), None);
  assert_eq!(find_query_param("a?a=1&url=value", url), None);
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
