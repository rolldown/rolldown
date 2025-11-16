use std::path::PathBuf;

use rolldown_plugin_vite_html_inline_proxy::ViteHtmlInlineProxyPlugin;
use sugar_path::SugarPath as _;

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug)]
pub struct BindingViteHtmlInlineProxyPluginConfig {
  pub root: String,
}

impl From<BindingViteHtmlInlineProxyPluginConfig> for ViteHtmlInlineProxyPlugin {
  fn from(value: BindingViteHtmlInlineProxyPluginConfig) -> Self {
    Self { root: PathBuf::from(value.root).normalize() }
  }
}
