use std::path::PathBuf;

use rolldown_plugin_vite_wasm_helper::{ViteWasmHelperPlugin, ViteWasmHelperPluginV2Config};
use sugar_path::SugarPath as _;

use crate::options::plugin::types::binding_asset_inline_limit::BindingAssetInlineLimit;

#[napi_derive::napi(object, object_to_js = false)]
pub struct BindingViteWasmHelperPluginV2Config {
  pub root: String,
  pub is_lib: bool,
  pub public_dir: String,
  #[napi(ts_type = "number | ((file: string, content: Buffer) => boolean | undefined)")]
  pub asset_inline_limit: BindingAssetInlineLimit,
}

#[napi_derive::napi(object, object_to_js = false)]
pub struct BindingViteWasmHelperPluginConfig {
  pub decoded_base: String,
  pub v2: Option<BindingViteWasmHelperPluginV2Config>,
}

impl From<BindingViteWasmHelperPluginConfig> for ViteWasmHelperPlugin {
  fn from(value: BindingViteWasmHelperPluginConfig) -> Self {
    Self {
      decoded_base: value.decoded_base,
      v2: value.v2.map(|v2| ViteWasmHelperPluginV2Config {
        root: PathBuf::from(v2.root).normalize(),
        is_lib: v2.is_lib,
        public_dir: v2.public_dir,
        asset_inline_limit: v2.asset_inline_limit.into(),
      }),
    }
  }
}
