use rolldown_plugin_vite_html::ViteHtmlPlugin;
use rolldown_utils::dashmap::FxDashMap;

use crate::options::plugin::types::{
  binding_asset_inline_limit::BindingAssetInlineLimit,
  binding_module_preload::BindingModulePreload, binding_render_built_url::BindingRenderBuiltUrl,
};

#[napi_derive::napi(object, object_to_js = false)]
pub struct BindingViteHtmlPluginConfig {
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
}

impl From<BindingViteHtmlPluginConfig> for ViteHtmlPlugin {
  fn from(value: BindingViteHtmlPluginConfig) -> Self {
    Self {
      is_lib: value.is_lib,
      is_ssr: value.is_ssr,
      url_base: value.url_base,
      public_dir: value.public_dir,
      decoded_base: value.decoded_base,
      css_code_split: value.css_code_split,
      module_preload: value.module_preload.into(),
      asset_inline_limit: value.asset_inline_limit.into(),
      render_built_url: value.render_built_url.map(Into::into),
      html_result_map: FxDashMap::default(),
    }
  }
}
