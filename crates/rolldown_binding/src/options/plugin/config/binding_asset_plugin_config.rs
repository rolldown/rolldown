use std::sync::Arc;

use derive_more::Debug;
use napi::bindgen_prelude::{Buffer, Either, FnArgs};
use napi_derive::napi;
use rolldown_plugin_asset::AssetPlugin;
use rolldown_plugin_utils::UsizeOrFunction;
use rolldown_plugin_utils::{RenderBuiltUrl, RenderBuiltUrlConfig, RenderBuiltUrlRet};
use rolldown_utils::dashmap::FxDashSet;

use crate::types::{
  binding_string_or_regex::{BindingStringOrRegex, bindingify_string_or_regex_array},
  js_callback::{
    JsCallback, JsCallbackExt as _, MaybeAsyncJsCallback, MaybeAsyncJsCallbackExt as _,
  },
};

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
  #[napi(ts_type = "number | ((file: string, content: Buffer) => boolean | undefined)")]
  pub asset_inline_limit: Option<Either<u32, JsCallback<FnArgs<(String, Buffer)>, Option<bool>>>>,
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
    let asset_inline_limit =
      config.asset_inline_limit.map(|asset_inline_limit| match asset_inline_limit {
        Either::A(value) => UsizeOrFunction::Number(value as usize),
        Either::B(func) => {
          UsizeOrFunction::Function(Arc::new(move |file: &str, content: &[u8]| {
            let file = file.to_string();
            let content = Buffer::from(content);
            let asset_inline_limit_fn = Arc::clone(&func);
            Box::pin(async move {
              asset_inline_limit_fn
                .invoke_async((file, content).into())
                .await
                .map_err(anyhow::Error::from)
            })
          }))
        }
      });

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
      asset_inline_limit: asset_inline_limit.unwrap_or_default(),
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
