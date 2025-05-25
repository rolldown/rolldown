use std::borrow::Cow;
use std::path::Path;

use rolldown_utils::{
  dataurl::encode_as_shortest_dataurl, mime::guess_mime, pattern_filter::normalize_path,
  url::clean_url,
};
use sugar_path::SugarPath as _;

use super::check_public_file::check_public_file;
use super::find_special_query::find_special_query;

pub struct FileToUrlEnv<'a> {
  pub root: &'a str,
  pub command: &'a str,
  pub url_base: &'a str,
  pub public_dir: &'a str,
}

// TODO(shulaoda): improve it
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

#[inline]
pub fn file_to_url(env: &FileToUrlEnv<'_>, id: &str) -> anyhow::Result<String> {
  if env.command == "serve" { file_to_dev_url(env, id, false) } else { file_to_built_url(env) }
}

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

  // TODO(shulaoda): align below logic
  // If is svg and it's inlined in build, also inline it in dev to match
  // the behavior in build due to quote handling differences.
  // if cleaned_id.ends_with(".svg") {}

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
