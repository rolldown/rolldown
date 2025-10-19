use std::sync::{Arc, atomic::AtomicBool};

use napi::{Either, bindgen_prelude::FnArgs};
use rolldown_plugin_utils::{RenderBuiltUrl, RenderBuiltUrlConfig};
use rolldown_plugin_vite_css_post::{CSSMinifyFn, IsLegacyFn, ViteCSSPostPlugin};

use crate::{
  options::plugin::config::binding_asset_plugin_config::{
    BindingRenderBuiltUrlConfig, BindingRenderBuiltUrlRet,
  },
  types::js_callback::{
    JsCallback, JsCallbackExt as _, MaybeAsyncJsCallback, MaybeAsyncJsCallbackExt as _,
  },
};

#[expect(clippy::struct_excessive_bools)]
#[napi_derive::napi(object, object_to_js = false)]
pub struct BindingViteCSSPostPluginConfig {
  pub is_lib: bool,
  pub is_ssr: bool,
  pub is_worker: bool,
  pub is_client: bool,
  pub css_code_split: bool,
  pub sourcemap: bool,
  pub assets_dir: String,
  pub url_base: String,
  pub decoded_base: String,
  pub lib_css_filename: Option<String>,
  #[napi(ts_type = "() => boolean")]
  pub is_legacy: Option<JsCallback<(), bool>>,
  #[napi(ts_type = "(css: string) => Promise<string>")]
  pub css_minify: Option<MaybeAsyncJsCallback<String, String>>,
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

impl From<BindingViteCSSPostPluginConfig> for ViteCSSPostPlugin {
  fn from(value: BindingViteCSSPostPluginConfig) -> Self {
    Self {
      is_lib: value.is_lib,
      is_ssr: value.is_ssr,
      is_worker: value.is_worker,
      is_client: value.is_client,
      css_code_split: value.css_code_split,
      sourcemap: value.sourcemap,
      assets_dir: value.assets_dir,
      url_base: value.url_base,
      decoded_base: value.decoded_base,
      lib_css_filename: value.lib_css_filename,
      is_legacy: value.is_legacy.map(|cb| -> Arc<IsLegacyFn> {
        Arc::new(move || {
          let is_legacy_fn = Arc::clone(&cb);
          Box::pin(async move { is_legacy_fn.invoke_async(()).await.map_err(anyhow::Error::from) })
        })
      }),
      render_built_url: value.render_built_url.map(|render_built_url| -> Arc<RenderBuiltUrl> {
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
      css_minify: value.css_minify.map(|css_minify| -> Arc<CSSMinifyFn> {
        Arc::new(move |css: String| {
          let css_minify = Arc::clone(&css_minify);
          Box::pin(async move { css_minify.await_call(css).await.map_err(anyhow::Error::from) })
        })
      }),
      has_emitted: AtomicBool::default(),
    }
  }
}
