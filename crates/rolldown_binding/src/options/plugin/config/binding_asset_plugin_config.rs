use rolldown_plugin_asset::AssetPlugin;
use rolldown_utils::dashmap::FxDashSet;

use crate::types::binding_string_or_regex::{
  BindingStringOrRegex, bindingify_string_or_regex_array,
};

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug, Default)]
pub struct BindingAssetPluginConfig {
  pub is_lib: Option<bool>,
  pub url_base: Option<String>,
  pub public_dir: Option<String>,
  pub assets_include: Option<Vec<BindingStringOrRegex>>,
  pub asset_inline_limit: Option<u32>,
}

impl From<BindingAssetPluginConfig> for AssetPlugin {
  fn from(config: BindingAssetPluginConfig) -> Self {
    Self {
      is_lib: config.is_lib.unwrap_or_default(),
      url_base: config.url_base.unwrap_or_default(),
      public_dir: config.public_dir.unwrap_or_default(),
      assets_include: config
        .assets_include
        .map(bindingify_string_or_regex_array)
        .unwrap_or_default(),
      asset_inline_limit: config.asset_inline_limit.unwrap_or(4096) as usize,
      handled_ids: FxDashSet::default(),
    }
  }
}
