use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;

use rolldown_utils::dashmap::FxDashMap;
use rolldown_utils::url::clean_url;
use rolldown_utils::{dataurl::encode_as_shortest_dataurl, mime::guess_mime};
use sugar_path::SugarPath;

use crate::{PublicFileToBuiltUrlEnv, remove_special_query};

use super::check_public_file::check_public_file;
use super::find_special_query::find_special_query;

const GIT_LFS_PREFIX: &[u8; 34] = b"version https://git-lfs.github.com";

#[derive(Default)]
pub struct AssetCache(pub FxDashMap<String, String>);

type AssetInlineLimitFn = dyn (Fn(&str, &[u8]) -> Pin<Box<dyn Future<Output = anyhow::Result<Option<bool>>> + Send + Sync>>)
  + Send
  + Sync;

const DEFAULT_ASSETS_INLINE_LIMIT: usize = 4096;

#[derive(Clone)]
pub enum UsizeOrFunction {
  Number(usize),
  Function(Arc<AssetInlineLimitFn>),
}

impl Default for UsizeOrFunction {
  fn default() -> Self {
    Self::Number(DEFAULT_ASSETS_INLINE_LIMIT)
  }
}

pub struct FileToUrlEnv<'a> {
  pub root: &'a PathBuf,
  pub is_lib: bool,
  pub public_dir: &'a str,
  pub asset_inline_limit: &'a UsizeOrFunction,
  pub ctx: &'a rolldown_plugin::PluginContext,
}

impl FileToUrlEnv<'_> {
  pub async fn file_to_url(&self, id: &str) -> anyhow::Result<String> {
    self.file_to_built_url(id, false, None).await
  }

  pub async fn file_to_built_url(
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

    let url = if self.should_inline(file, &id, &content, force_inline).await? {
      self.asset_to_data_url(file.as_path(), &content)?
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

  async fn should_inline(
    &self,
    file: &str,
    id: &str,
    content: &[u8],
    force_inline: Option<bool>,
  ) -> anyhow::Result<bool> {
    if find_special_query(id, b"no-inline").is_some() {
      return Ok(false);
    }
    if self.is_lib || find_special_query(id, b"inline").is_some() {
      return Ok(true);
    }
    if let Some(module_info) = self.ctx.get_module_info(id) {
      if module_info.is_entry {
        return Ok(false);
      }
    }
    if let Some(force_inline) = force_inline {
      return Ok(force_inline);
    }
    if file.ends_with(".html") || (file.ends_with(".svg") && id.contains('#')) {
      return Ok(false);
    }
    let limit = match self.asset_inline_limit {
      UsizeOrFunction::Number(limit) => *limit,
      UsizeOrFunction::Function(func) => match func(file, content).await? {
        Some(user_should_inline) => return Ok(user_should_inline),
        None => DEFAULT_ASSETS_INLINE_LIMIT,
      },
    };
    Ok(content.len() < limit && !content.starts_with(GIT_LFS_PREFIX))
  }

  fn asset_to_data_url(&self, path: &Path, content: &[u8]) -> anyhow::Result<String> {
    if self.is_lib && content.starts_with(GIT_LFS_PREFIX) {
      self.ctx.warn(rolldown_plugin::LogWithoutPlugin {
        message: format!("Inlined file {} was not downloaded via Git LFS", path.display()),
        ..Default::default()
      });
    }
    // TODO: It needs to be validated during subsequent usage
    // https://github.com/vitejs/vite/pull/14643/files#r1376247460
    // https://github.com/vitejs/rolldown-vite/blob/c252dee/packages/vite/src/node/plugins/asset.ts#L533-L539
    let guessed_mime = guess_mime(path, content)?;
    Ok(encode_as_shortest_dataurl(&guessed_mime, content))
  }
}
