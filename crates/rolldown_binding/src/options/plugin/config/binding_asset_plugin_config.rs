use rolldown_plugin_asset::AssetPlugin;
use rolldown_utils::dashmap::FxDashSet;

use crate::options::plugin::types::binding_asset_inline_limit::BindingAssetInlineLimit;
use crate::options::plugin::types::binding_render_built_url::BindingRenderBuiltUrl;
use crate::types::binding_string_or_regex::{
  BindingStringOrRegex, bindingify_string_or_regex_array,
};

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
  pub asset_inline_limit: BindingAssetInlineLimit,
  #[napi(
    ts_type = "(filename: string, type: BindingRenderBuiltUrlConfig) => Promise<undefined | string | BindingRenderBuiltUrlRet>"
  )]
  pub render_built_url: Option<BindingRenderBuiltUrl>,
}

impl From<BindingAssetPluginConfig> for AssetPlugin {
  fn from(config: BindingAssetPluginConfig) -> Self {
    Self {
      is_lib: config.is_lib,
      is_ssr: config.is_ssr,
      is_worker: config.is_worker,
      url_base: config.url_base,
      public_dir: config.public_dir,
      decoded_base: config.decoded_base,
      is_skip_assets: config.is_skip_assets,
      assets_include: bindingify_string_or_regex_array(config.assets_include),
      asset_inline_limit: config.asset_inline_limit.into(),
      render_built_url: config.render_built_url.map(Into::into),
      handled_asset_ids: FxDashSet::default(),
    }
  }
}
