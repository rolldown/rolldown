use std::{path::Path, pin::Pin};

use itertools::Either;
use sugar_path::SugarPath as _;

use crate::join_url_segments;

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

pub type RenderBuiltUrl = dyn Fn(
    &str,
    &RenderBuiltUrlConfig,
  ) -> Pin<
    Box<(dyn Future<Output = anyhow::Result<Option<Either<String, RenderBuiltUrlRet>>>> + Send)>,
  > + Send
  + Sync;

pub struct ToOutputFilePathInJSEnv<'a> {
  pub base: &'a str,
  pub decoded_base: &'a str,
  pub render_built_url: Option<&'a RenderBuiltUrl>,
}

pub async fn to_output_file_path_in_js<'a>(
  env: ToOutputFilePathInJSEnv<'a>,
  config: RenderBuiltUrlConfig<'a>,
  filename: &str,
  to_relative: Either<impl Fn(&Path, &Path) -> String, impl Fn(&Path, &Path) -> String>,
) -> anyhow::Result<Either<String, String>> {
  let mut relative = env.base.is_empty() || env.base.starts_with("./");
  if let Some(render_built_url) = env.render_built_url {
    if let Some(result) = render_built_url(filename, &config).await? {
      match result {
        Either::Left(result) => return Ok(Either::Left(result)),
        Either::Right(result) => {
          if let Some(runtime) = result.runtime {
            return Ok(Either::Right(runtime));
          }
          if let Some(r) = result.relative {
            relative = r;
          }
        }
      }
    }
  }
  if relative && !config.is_server {
    return Ok(match to_relative {
      Either::Left(to_relative) => {
        Either::Left(to_relative(filename.as_path(), config.host_id.as_path()))
      }
      Either::Right(to_relative) => {
        Either::Right(to_relative(filename.as_path(), config.host_id.as_path()))
      }
    });
  }
  Ok(Either::Left(join_url_segments(env.decoded_base, filename).into_owned()))
}
