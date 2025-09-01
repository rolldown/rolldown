use derive_more::Debug;
use rolldown_plugin_esm_external_require::EsmExternalRequirePlugin;

use crate::types::binding_string_or_regex::{
  BindingStringOrRegex, bindingify_string_or_regex_array,
};

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug, Default)]
pub struct BindingEsmExternalRequirePluginConfig {
  pub external: Vec<BindingStringOrRegex>,
}

impl From<BindingEsmExternalRequirePluginConfig> for EsmExternalRequirePlugin {
  fn from(config: BindingEsmExternalRequirePluginConfig) -> Self {
    Self { external: bindingify_string_or_regex_array(config.external) }
  }
}
