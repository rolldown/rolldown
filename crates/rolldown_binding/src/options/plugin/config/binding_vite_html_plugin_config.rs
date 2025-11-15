use std::{path::PathBuf, sync::Arc};

use napi::bindgen_prelude::FnArgs;
use rolldown_common::{Output, OutputChunk};
use rolldown_plugin_vite_html::{TransformIndexHtml, ViteHtmlPlugin};
use rolldown_utils::dashmap::FxDashMap;
use sugar_path::SugarPath as _;

use crate::{
  options::plugin::types::{
    binding_asset_inline_limit::BindingAssetInlineLimit,
    binding_module_preload::BindingModulePreload, binding_render_built_url::BindingRenderBuiltUrl,
  },
  types::{
    binding_output_chunk::BindingOutputChunk,
    binding_outputs::BindingOutputs,
    js_callback::{MaybeAsyncJsCallback, MaybeAsyncJsCallbackExt as _},
  },
};

#[napi_derive::napi(object, object_to_js = false)]
pub struct BindingViteHtmlPluginConfig {
  pub root: String,
  pub is_lib: bool,
  pub is_ssr: bool,
  pub url_base: String,
  pub public_dir: String,
  pub decoded_base: String,
  pub css_code_split: bool,
  #[napi(ts_type = "false | BindingModulePreloadOptions")]
  pub module_preload: BindingModulePreload,
  #[napi(ts_type = "number | ((file: string, content: Buffer) => boolean | undefined)")]
  pub asset_inline_limit: BindingAssetInlineLimit,
  #[napi(
    ts_type = "(filename: string, type: BindingRenderBuiltUrlConfig) => undefined | string | BindingRenderBuiltUrlRet"
  )]
  pub render_built_url: Option<BindingRenderBuiltUrl>,
  #[napi(
    ts_type = "(html: string, path: string, filename: string, hook: 'transform' | 'generateBundle', output?: BindingOutputs, chunk?: BindingOutputChunk) => Promise<string>"
  )]
  pub transform_index_html: MaybeAsyncJsCallback<
    FnArgs<(String, String, String, String, Option<BindingOutputs>, Option<BindingOutputChunk>)>,
    String,
  >,
}

impl From<BindingViteHtmlPluginConfig> for ViteHtmlPlugin {
  fn from(value: BindingViteHtmlPluginConfig) -> Self {
    let transform_index_html: Arc<TransformIndexHtml> = Arc::new(
      move |html: &str,
            path: &str,
            filename: &str,
            output: Option<Vec<Output>>,
            chunk: Option<Arc<OutputChunk>>,
            hook: &'static str| {
        let html = html.to_string();
        let path = path.to_string();
        let hook = hook.to_string();
        let filename = filename.to_string();
        let cb = Arc::clone(&value.transform_index_html);
        Box::pin(async move {
          cb.await_call(
            (
              html,
              path,
              filename,
              hook,
              output.map(Into::into),
              chunk.map(BindingOutputChunk::new),
            )
              .into(),
          )
          .await
          .map_err(anyhow::Error::from)
        })
      },
    );

    Self {
      root: PathBuf::from(value.root).normalize(),
      is_lib: value.is_lib,
      is_ssr: value.is_ssr,
      url_base: value.url_base,
      public_dir: value.public_dir,
      decoded_base: value.decoded_base,
      css_code_split: value.css_code_split,
      module_preload: value.module_preload.into(),
      asset_inline_limit: value.asset_inline_limit.into(),
      render_built_url: value.render_built_url.map(Into::into),
      transform_index_html,
      html_result_map: FxDashMap::default(),
    }
  }
}
