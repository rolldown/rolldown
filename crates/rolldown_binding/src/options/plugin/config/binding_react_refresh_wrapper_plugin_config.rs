use crate::types::binding_string_or_regex::{
  BindingStringOrRegex, bindingify_string_or_regex_array,
};
use rolldown_plugin_react_refresh_wrapper::ReactRefreshWrapperPluginOptions;

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug, Default)]
pub struct BindingReactRefreshWrapperPluginConfig {
  pub cwd: String,
  pub include: Option<Vec<BindingStringOrRegex>>,
  pub exclude: Option<Vec<BindingStringOrRegex>>,
  pub jsx_import_source: String,
  pub react_refresh_host: String,
}

impl From<BindingReactRefreshWrapperPluginConfig> for ReactRefreshWrapperPluginOptions {
  fn from(value: BindingReactRefreshWrapperPluginConfig) -> Self {
    Self {
      cwd: value.cwd,
      include: value.include.map(bindingify_string_or_regex_array).unwrap_or_default(),
      exclude: value.exclude.map(bindingify_string_or_regex_array).unwrap_or_default(),
      jsx_import_source: value.jsx_import_source,
      react_refresh_host: value.react_refresh_host,
    }
  }
}
