use std::sync::{Arc, atomic::AtomicBool};

use rolldown_plugin_vite_css_post::{CSSMinifyFn, IsLegacyFn, ViteCSSPostPlugin};

use crate::{
  options::plugin::types::binding_render_built_url::BindingRenderBuiltUrl,
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
    ts_type = "(filename: string, type: BindingRenderBuiltUrlConfig) => Promise<undefined | string | BindingRenderBuiltUrlRet>"
  )]
  pub render_built_url: Option<BindingRenderBuiltUrl>,
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
      render_built_url: value.render_built_url.map(Into::into),
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
