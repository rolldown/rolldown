use std::sync::Arc;

use napi::bindgen_prelude::{Buffer, Either, FnArgs};
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

#[napi_derive::napi(object, object_from_js = false)]
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

#[napi_derive::napi(object, object_to_js = false)]
pub struct BindingRenderBuiltUrlRet {
  pub relative: Option<bool>,
  pub runtime: Option<String>,
}

impl From<BindingRenderBuiltUrlRet> for RenderBuiltUrlRet {
  fn from(value: BindingRenderBuiltUrlRet) -> Self {
    Self { relative: value.relative, runtime: value.runtime }
  }
}

#[expect(clippy::struct_excessive_bools)]
#[napi_derive::napi(object, object_to_js = false)]
pub struct BindingAssetPluginConfig {
  pub is_lib: bool,
  pub is_ssr: bool,
  pub is_worker: bool,
  pub url_base: String,
  pub public_dir: String,
  pub decoded_base: String,
  pub is_skip_assets: bool,
  pub assets_include: Vec<BindingStringOrRegex>,
  #[napi(ts_type = "number | ((file: string, content: Buffer) => boolean | undefined)")]
  pub asset_inline_limit: Either<u32, JsCallback<FnArgs<(String, Buffer)>, Option<bool>>>,
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
    let asset_inline_limit = match config.asset_inline_limit {
      Either::A(value) => UsizeOrFunction::Number(value as usize),
      Either::B(func) => UsizeOrFunction::Function(Arc::new(move |file: &str, content: &[u8]| {
        let file = file.to_string();
        let content = Buffer::from(content);
        let asset_inline_limit_fn = Arc::clone(&func);
        Box::pin(async move {
          asset_inline_limit_fn
            .invoke_async((file, content).into())
            .await
            .map_err(anyhow::Error::from)
        })
      })),
    };

    Self {
      is_lib: config.is_lib,
      is_ssr: config.is_ssr,
      is_worker: config.is_worker,
      url_base: config.url_base,
      public_dir: config.public_dir,
      decoded_base: config.decoded_base,
      is_skip_assets: config.is_skip_assets,
      assets_include: bindingify_string_or_regex_array(config.assets_include),
      asset_inline_limit,
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
