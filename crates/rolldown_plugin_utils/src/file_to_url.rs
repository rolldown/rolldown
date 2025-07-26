use std::borrow::Cow;
use std::path::Path;

use rolldown_utils::{dataurl::encode_as_shortest_dataurl, mime::guess_mime};
use sugar_path::SugarPath as _;

use crate::PublicFileToBuiltUrlEnv;

use super::check_public_file::check_public_file;
use super::find_special_query::find_special_query;

pub struct FileToUrlEnv<'a> {
  pub root: &'a str,
  pub url_base: &'a str,
  pub public_dir: &'a str,
  pub ctx: &'a rolldown_plugin::PluginContext,
}

impl FileToUrlEnv<'_> {
  pub fn file_to_url(&self, id: &str) -> anyhow::Result<String> {
    self.file_to_built_url(id, false, false)
  }

  #[allow(unused_assignments)]
  fn file_to_built_url(
    &self,
    id: &str,
    skip_public_check: bool,
    _force_inline: bool,
  ) -> anyhow::Result<String> {
    let mut id = Cow::Borrowed(id);
    if !skip_public_check {
      if let Some(public_file) = check_public_file(&id, self.public_dir) {
        if find_special_query(&id, b"inline").is_some() {
          id = public_file.to_slash_lossy();
        } else {
          let env = PublicFileToBuiltUrlEnv { ctx: self.ctx };
          return Ok(env.public_file_to_built_url(&id));
        }
      }
    }

    todo!()
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
