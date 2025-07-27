use std::borrow::Cow;
use std::path::{Path, PathBuf};

use rolldown_utils::dashmap::FxDashMap;
use rolldown_utils::url::clean_url;
use rolldown_utils::{dataurl::encode_as_shortest_dataurl, mime::guess_mime};
use sugar_path::SugarPath;

use crate::{PublicFileToBuiltUrlEnv, remove_special_query};

use super::check_public_file::check_public_file;
use super::find_special_query::find_special_query;

#[derive(Default)]
pub struct AssetCache(pub FxDashMap<String, String>);

pub struct FileToUrlEnv<'a> {
  pub root: &'a PathBuf,
  pub is_lib: bool,
  pub url_base: &'a str,
  pub public_dir: &'a str,
  pub asset_inline_limit: usize,
  pub ctx: &'a rolldown_plugin::PluginContext,
}

impl FileToUrlEnv<'_> {
  pub async fn file_to_url(&self, id: &str) -> anyhow::Result<String> {
    self.file_to_built_url(id, false, None).await
  }

  async fn file_to_built_url(
    &self,
    id: &str,
    skip_public_check: bool,
    force_inline: Option<bool>,
  ) -> anyhow::Result<String> {
    let mut id = Cow::Borrowed(id);
    if !skip_public_check {
      if let Some(public_file) = check_public_file(&id, self.public_dir) {
        if find_special_query(&id, b"inline").is_some() {
          id = Cow::Owned(public_file.to_slash_lossy().into_owned());
        } else {
          let env = PublicFileToBuiltUrlEnv { ctx: self.ctx };
          return Ok(env.public_file_to_built_url(&id));
        }
      }
    }

    let cache =
      self.ctx.meta().get::<AssetCache>().ok_or_else(|| anyhow::anyhow!("AssetCache missing"))?;
    if let Some(cached) = cache.0.get(id.as_ref()) {
      return Ok(cached.to_string());
    }

    let file = clean_url(&id);
    let content = std::fs::read(file)?;

    let url = if self.should_inline(file, &id, &content, force_inline) {
      asset_to_data_url(file.as_path(), &content)?
    } else {
      let path = Path::new(file);
      let name = path.file_name().map(|v| v.to_string_lossy().into());
      let original_file_name = path.relative(self.root).to_string_lossy().into_owned();
      let emitted_asset = rolldown_common::EmittedAsset {
        name,
        source: content.into(),
        original_file_name: Some(original_file_name),
        ..Default::default()
      };

      let reference_id = self.ctx.emit_file_async(emitted_asset).await?;
      let postfix = if file.len() == id.len() {
        ""
      } else {
        let postfix = remove_special_query(&id[file.len()..], b"no-inline");
        &rolldown_utils::concat_string!("$_", postfix, "__")
      };
      rolldown_utils::concat_string!("__VITE_ASSET__", reference_id, "__", postfix)
    };

    cache.0.insert(id.to_string(), url.clone());
    Ok(url)
  }

  #[allow(clippy::case_sensitive_file_extension_comparisons)]
  fn should_inline(
    &self,
    file: &str,
    id: &str,
    content: &[u8],
    force_inline: Option<bool>,
  ) -> bool {
    if find_special_query(id, b"no-inline").is_some() {
      return false;
    }
    if self.is_lib || find_special_query(id, b"inline").is_some() {
      return true;
    }
    if let Some(module_info) = self.ctx.get_module_info(id) {
      if module_info.is_entry {
        return false;
      }
    }
    if let Some(force_inline) = force_inline {
      return force_inline;
    }
    if file.ends_with(".html") || (file.ends_with(".svg") && id.contains('#')) {
      return false;
    }
    // TODO(shulaoda): support function for asset_inline_limit
    content.len() < self.asset_inline_limit && !is_git_lfs_placeholder(content)
  }
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

const GIT_LFS_PREFIX: &[u8; 34] = b"version https://git-lfs.github.com";
fn is_git_lfs_placeholder(content: &[u8]) -> bool {
  if content.len() < GIT_LFS_PREFIX.len() {
    return false;
  }
  content[..GIT_LFS_PREFIX.len()] == *GIT_LFS_PREFIX
}

#[test]
fn test_is_git_lfs_placeholder() {
  assert!(is_git_lfs_placeholder(b"version https://git-lfs.github.com/spec/v1"));
  assert!(!is_git_lfs_placeholder(b"version https:"));
  assert!(!is_git_lfs_placeholder(b"https://www.xgz.com/spec./yyy"));
}
