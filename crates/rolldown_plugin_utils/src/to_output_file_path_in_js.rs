use std::{path::Path, pin::Pin};

use itertools::Either;
use sugar_path::SugarPath as _;

use crate::join_url_segments;

pub type RenderBuiltUrl = dyn Fn(
    &str,
    &RenderBuiltUrlConfig,
  ) -> Pin<
    Box<(dyn Future<Output = anyhow::Result<Option<Either<String, RenderBuiltUrlRet>>>> + Send)>,
  > + Send
  + Sync;

pub struct RenderBuiltUrlConfig<'a> {
  pub r#type: &'a str,
  pub host_id: &'a str,
  pub host_type: &'a str,
  pub is_server: bool,
}

pub struct RenderBuiltUrlRet {
  relative: Option<bool>,
  runtime: Option<String>,
}

pub enum AssetUrlResult {
  WithRuntime(String),
  WithoutRuntime(String),
}

pub struct ToOutputFilePathInJSEnv<'a> {
  pub base: &'a str,
  pub decoded_base: &'a str,
  pub render_built_url: Option<&'a RenderBuiltUrl>,
  pub render_built_url_config: RenderBuiltUrlConfig<'a>,
}

impl ToOutputFilePathInJSEnv<'_> {
  pub async fn to_output_file_path_in_js(
    &self,
    filename: &str,
    to_relative: impl Fn(&Path, &Path) -> AssetUrlResult,
  ) -> anyhow::Result<AssetUrlResult> {
    let mut relative = self.base.is_empty() || self.base.starts_with("./");
    if let Some(render_built_url) = self.render_built_url {
      if let Some(result) = render_built_url(filename, &self.render_built_url_config).await? {
        match result {
          Either::Left(result) => return Ok(AssetUrlResult::WithoutRuntime(result)),
          Either::Right(result) => {
            if let Some(runtime) = result.runtime {
              return Ok(AssetUrlResult::WithRuntime(runtime));
            }
            if let Some(r) = result.relative {
              relative = r;
            }
          }
        }
      }
    }
    if relative && !self.render_built_url_config.is_server {
      return Ok(to_relative(filename.as_path(), self.render_built_url_config.host_id.as_path()));
    }
    Ok(AssetUrlResult::WithoutRuntime(join_url_segments(self.decoded_base, filename).into_owned()))
  }
}
