use std::borrow::Cow;
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use rolldown_plugin::PluginContext;
use rolldown_utils::{
  dataurl::encode_as_shortest_dataurl, mime::guess_mime, pattern_filter::normalize_path,
  url::clean_url,
};
use sugar_path::SugarPath as _;

use crate::PublicFileToBuiltUrlEnv;

use super::check_public_file::check_public_file;
use super::find_special_query::find_special_query;

static NO_INLINE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[?&]no-inline\b").unwrap());
static INLINE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[?&]inline\b").unwrap());
pub struct FileToUrlEnv<'a> {
  pub root: &'a str,
  pub url_base: &'a str,
  pub public_dir: &'a str,
  pub asset_inline_limit: usize,
  pub ctx: Option<&'a PluginContext>,
}

// TODO(shulaoda): improve it
#[allow(dead_code)]
fn asset_to_data_url(path: &Path, content: &[u8]) -> anyhow::Result<String> {
  // TODO(shulaoda): should throw an warning
  // if (environment.config.build.lib && isGitLfsPlaceholder(content)) {
  //   environment.logger.warn(
  //     colors.yellow(`Inlined file ${file} was not downloaded via Git LFS`),
  //   )
  // }
  let guessed_mime = guess_mime(path, content)?;
  Ok(encode_as_shortest_dataurl(&guessed_mime, content))
}

#[allow(dead_code)]
pub fn file_to_dev_url(
  env: &FileToUrlEnv<'_>,
  id: &str,
  skip_base: bool,
) -> anyhow::Result<String> {
  let public_file = check_public_file(id, env.public_dir)
    .map(|file| Cow::Owned(file.to_slash_lossy().into_owned()));

  // If has inline query, unconditionally inline the asset
  if find_special_query(id, b"inline").is_some() {
    let file = public_file.unwrap_or(Cow::Borrowed(clean_url(id)));
    let content = std::fs::read_to_string(&*file)?;
    return asset_to_data_url(file.as_path(), content.as_bytes());
  }

  let cleaned_id = clean_url(id);
  if cleaned_id.ends_with(".svg") {
    let temp_file = Cow::Borrowed(cleaned_id);
    let file = public_file.as_ref().unwrap_or(&temp_file);
    let content = std::fs::read_to_string(&**file)?;
    if should_inline(env, file, id, content.as_bytes(), None, None) {
      return asset_to_data_url(file.as_path(), content.as_bytes());
    }
  }

  let url = if public_file.is_some() {
    id /* must start with '/', see check_public_file */
  } else {
    let path = Path::new(id);
    &if path.starts_with(env.root) {
      format!("/{}", path.relative(env.root).to_slash_lossy())
    } else {
      format!("/@fs/{}", normalize_path(id))
    }
  };

  if skip_base {
    return Ok(url.to_string());
  }

  let stripped_url = &url[1..] /* remove leading slash */;
  Ok(if env.url_base.is_empty() {
    stripped_url.to_string()
  } else {
    format!("{}/{stripped_url}", env.url_base.strip_suffix('/').unwrap_or(env.url_base))
  })
}

fn file_to_built_url(_env: &FileToUrlEnv<'_>) -> anyhow::Result<String> {
  todo!()
}

fn should_inline(
  env: &FileToUrlEnv<'_>,
  file: &str,
  id: &str,
  content: &[u8],
  ctx: Option<&PluginContext>,
  force_inline: Option<bool>,
) -> bool {
  if NO_INLINE_RE.is_match(id) {
    return false;
  }
  if INLINE_RE.is_match(id) {
    return true;
  }

  if let Some(ctx) = ctx {
    if ctx.get_module_info(id).unwrap().is_entry {
      return false;
    }
  }

  if force_inline.is_some() {
    return force_inline.unwrap();
  }

  if file.ends_with(".html") {
    return false;
  }
  if file.ends_with(".svg") && id.contains("#") {
    return false;
  }

  return content.len() < env.asset_inline_limit;
}

fn is_git_lfs_placeholder(content: &[u8]) -> bool {
  let git_lfs = b"version https://git-lfs.github.com";
  if content.len() < git_lfs.len() {
    return false;
  }
  return content[..git_lfs.len()] == *git_lfs;
}

#[test]
fn test_is_git_lfs_placeholder() {
  assert!(is_git_lfs_placeholder(b"version https://git-lfs.github.com/spec/v1"));
  assert!(!is_git_lfs_placeholder(b"version https:"));
  assert!(!is_git_lfs_placeholder(b"https://www.xgz.com/spec./yyy"));
}
