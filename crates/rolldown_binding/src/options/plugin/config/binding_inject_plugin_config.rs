use rolldown_plugin_inject::{InjectPlugin, InjectPluginOptions};

use crate::options::binding_inject_import::{BindingInjectImport, normalize_binding_inject_import};
use crate::types::binding_string_or_regex::{
  BindingStringOrRegex, bindingify_string_or_regex_array,
};

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Default)]
pub struct BindingInjectPluginConfig {
  pub values: Vec<BindingInjectImport>,
  pub include: Option<Vec<BindingStringOrRegex>>,
  pub exclude: Option<Vec<BindingStringOrRegex>>,
}

impl From<BindingInjectPluginConfig> for InjectPlugin {
  fn from(config: BindingInjectPluginConfig) -> Self {
    InjectPlugin::new(InjectPluginOptions {
      injects: config.values.into_iter().map(normalize_binding_inject_import).collect(),
      include: config.include.map(bindingify_string_or_regex_array).unwrap_or_default(),
      exclude: config.exclude.map(bindingify_string_or_regex_array).unwrap_or_default(),
    })
  }
}
