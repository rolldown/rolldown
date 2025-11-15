use std::path::PathBuf;

use rolldown_plugin_html_inline_proxy::HtmlInlineProxyPlugin;
use sugar_path::SugarPath as _;

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug, Default)]
pub struct BindingHtmlInlineProxyPluginConfig {
  pub root: String,
}

impl From<BindingHtmlInlineProxyPluginConfig> for HtmlInlineProxyPlugin {
  fn from(value: BindingHtmlInlineProxyPluginConfig) -> Self {
    Self { root: PathBuf::from(value.root).normalize() }
  }
}
