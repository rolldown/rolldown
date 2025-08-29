use std::{path::Path, pin::Pin};

use itertools::Either;
use sugar_path::SugarPath as _;

use super::{join_url_segments, uri::encode_uri_path};

pub type RenderBuiltUrl = dyn Fn(
    &str,
    &RenderBuiltUrlConfig,
  ) -> Pin<
    Box<(dyn Future<Output = anyhow::Result<Option<Either<String, RenderBuiltUrlRet>>>> + Send)>,
  > + Send
  + Sync;

pub struct RenderBuiltUrlConfig<'a> {
  pub is_ssr: bool,
  pub r#type: &'a str,
  pub host_id: &'a str,
  pub host_type: &'a str,
}

pub struct RenderBuiltUrlRet {
  pub relative: Option<bool>,
  pub runtime: Option<String>,
}

pub enum AssetUrlResult {
  WithRuntime(String),
  WithoutRuntime(String),
}

impl AssetUrlResult {
  pub fn to_asset_url_in_js(self) -> anyhow::Result<String> {
    match self {
      AssetUrlResult::WithRuntime(v) => Ok(rolldown_utils::concat_string!("\"+", v, "+\"")),
      AssetUrlResult::WithoutRuntime(v) => {
        let string = serde_json::to_string(&encode_uri_path(v))?;
        Ok(string[1..string.len() - 1].to_owned())
      }
    }
  }

  pub fn to_asset_url_in_css_or_html(self) -> String {
    match self {
      AssetUrlResult::WithRuntime(_) => unreachable!("The asset url should not have a runtime"),
      AssetUrlResult::WithoutRuntime(v) => v,
    }
  }
}

pub struct ToOutputFilePathEnv<'a> {
  pub url_base: &'a str,
  pub decoded_base: &'a str,
  pub render_built_url: Option<&'a RenderBuiltUrl>,
  pub render_built_url_config: RenderBuiltUrlConfig<'a>,
}

impl ToOutputFilePathEnv<'_> {
  pub async fn to_output_file_path(
    &self,
    filename: &str,
    to_relative: impl Fn(&Path, &Path) -> AssetUrlResult,
  ) -> anyhow::Result<AssetUrlResult> {
    let mut relative = self.url_base.is_empty() || self.url_base == "./";
    if let Some(render_built_url) = self.render_built_url {
      if let Some(result) = render_built_url(filename, &self.render_built_url_config).await? {
        match result {
          Either::Left(result) => return Ok(AssetUrlResult::WithoutRuntime(result)),
          Either::Right(result) => {
            if let Some(runtime) = result.runtime {
              if matches!(self.render_built_url_config.host_type, "css" | "html") {
                return Err(anyhow::anyhow!(
                  "The `{{ runtime: '{runtime}' }}` is not supported for assets in {} files: {filename}",
                  self.render_built_url_config.host_type
                ));
              }
              return Ok(AssetUrlResult::WithRuntime(runtime));
            }
            if let Some(r) = result.relative {
              relative = r;
            }
          }
        }
      }
    }
    Ok(if relative && !self.render_built_url_config.is_ssr {
      to_relative(filename.as_path(), self.render_built_url_config.host_id.as_path())
    } else {
      AssetUrlResult::WithoutRuntime(join_url_segments(self.decoded_base, filename).into_owned())
    })
  }
}
