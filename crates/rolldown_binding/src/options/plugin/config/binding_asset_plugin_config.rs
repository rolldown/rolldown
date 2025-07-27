use std::sync::Arc;

use napi::bindgen_prelude::FnArgs;
use std::sync::Arc;

use derive_more::Debug;
use napi::bindgen_prelude::{Either, FnArgs};
use napi_derive::napi;
use rolldown_plugin_asset::AssetPlugin;
use rolldown_plugin_utils::UsizeOrFunction;
use rolldown_plugin_utils::{RenderBuiltUrl, RenderBuiltUrlConfig, RenderBuiltUrlRet};
use rolldown_utils::dashmap::FxDashSet;

use crate::types::{
  binding_string_or_regex::{BindingStringOrRegex, bindingify_string_or_regex_array},
  js_callback::{JsCallback, JsCallbackExt},
};
use derive_more::Debug;
type BindingUsizeOrFunction = JsCallback<FnArgs<(String, String)>, Option<u32>>;

#[napi(object)]
pub struct BindingRenderBuiltUrlConfig {
  pub ssr: bool,
  #[napi(ts_type = "'asset' | 'public'")]
  pub r#type: String,
  pub host_id: String,
  #[napi(ts_type = "'js' | 'css' | 'html'")]
  pub host_type: String,
}

impl From<&RenderBuiltUrlConfig<'_>> for BindingRenderBuiltUrlConfig {
  fn from(value: &RenderBuiltUrlConfig) -> Self {
    Self {
      ssr: value.is_ssr,
      r#type: value.r#type.to_string(),
      host_id: value.host_id.to_string(),
      host_type: value.host_type.to_string(),
    }
  }
}

#[napi(object)]
pub struct BindingRenderBuiltUrlRet {
  pub relative: Option<bool>,
  pub runtime: Option<String>,
}

impl From<BindingRenderBuiltUrlRet> for RenderBuiltUrlRet {
  fn from(value: BindingRenderBuiltUrlRet) -> Self {
    Self { relative: value.relative, runtime: value.runtime }
  }
}

#[napi(object, object_to_js = false)]
#[derive(Debug)]
pub struct BindingAssetPluginConfig {
  pub is_lib: Option<bool>,
  pub is_ssr: Option<bool>,
  pub is_worker: Option<bool>,
  pub url_base: Option<String>,
  pub public_dir: Option<String>,
  pub decoded_base: Option<String>,
  pub is_skip_assets: Option<bool>,
  pub assets_include: Option<Vec<BindingStringOrRegex>>,
  #[debug(skip)]
  #[napi(ts_type = "number |(file: string, content:String)=> VoidNullable<number>")]
  pub asset_inline_limit: napi::Either<u32, BindingUsizeOrFunction>,
  #[debug(skip)]
  #[napi(
    ts_type = "(filename: string, type: BindingRenderBuiltUrlConfig) => MaybePromise<VoidNullable<string | BindingRenderBuiltUrlRet>>"
  )]
  pub render_built_url: Option<
    MaybeAsyncJsCallback<
      FnArgs<(String, BindingRenderBuiltUrlConfig)>,
      Option<Either<String, BindingRenderBuiltUrlRet>>,
    >,
  >,
}

impl From<BindingAssetPluginConfig> for AssetPlugin {
  fn from(config: BindingAssetPluginConfig) -> Self {
    let assert_inline_limit = match config.asset_inline_limit {
      napi::Either::A(limit) => UsizeOrFunction::Number(limit as usize),
      napi::Either::B(func) => {
        UsizeOrFunction::Function(Arc::new(move |file: &str, content: &[u8]| {
          let file = file.to_string();
          let content = String::from_utf8_lossy(content).to_string();
          Box::pin({
            let value = func.clone();
            async move {
              match value.invoke_async((file, content).into()).await {
                Ok(Some(value)) => Ok(Some(value as usize)),
                Ok(None) => Ok(None),
                Err(e) => Err(anyhow::Error::from(e)),
              }
            }
          })
        }))
      }
    };
    Self {
      is_lib: config.is_lib.unwrap_or_default(),
      is_ssr: config.is_ssr.unwrap_or_default(),
      is_worker: config.is_worker.unwrap_or_default(),
      url_base: config.url_base.unwrap_or_default(),
      public_dir: config.public_dir.unwrap_or_default(),
      decoded_base: config.decoded_base.unwrap_or_default(),
      is_skip_assets: config.is_skip_assets.unwrap_or_default(),
      assets_include: config
        .assets_include
        .map(bindingify_string_or_regex_array)
        .unwrap_or_default(),
      asset_inline_limit: assert_inline_limit,
      render_built_url: config.render_built_url.map(|render_built_url| -> Arc<RenderBuiltUrl> {
        Arc::new(move |filename: &str, config: &RenderBuiltUrlConfig| {
          let render_built_url = Arc::clone(&render_built_url);
          let filename = filename.to_string();
          let config = config.into();
          Box::pin(async move {
            render_built_url
              .await_call((filename, config).into())
              .await
              .map(|v| {
                v.map(|v| match v {
                  Either::A(v) => itertools::Either::Left(v),
                  Either::B(v) => itertools::Either::Right(v.into()),
                })
              })
              .map_err(anyhow::Error::from)
          })
        })
      }),
      handled_asset_ids: FxDashSet::default(),
    }
  }
}
