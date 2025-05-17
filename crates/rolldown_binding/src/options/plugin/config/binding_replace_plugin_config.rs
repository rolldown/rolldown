use std::collections::HashMap;

use rolldown_plugin_replace::ReplaceOptions;
use rustc_hash::FxBuildHasher;

#[napi_derive::napi(object)]
#[derive(Debug, Default)]
pub struct BindingReplacePluginConfig {
  // It's ok we use `HashMap` here, because we don't care about the order of the keys.
  pub values: HashMap<String, String, FxBuildHasher>,
  #[napi(ts_type = "[string, string]")]
  pub delimiters: Option<Vec<String>>,
  pub prevent_assignment: Option<bool>,
  pub object_guards: Option<bool>,
  pub sourcemap: Option<bool>,
}

impl From<BindingReplacePluginConfig> for ReplaceOptions {
  fn from(config: BindingReplacePluginConfig) -> Self {
    Self {
      values: config.values,
      delimiters: config.delimiters.map(|raw| (raw[0].clone(), raw[1].clone())),
      prevent_assignment: config.prevent_assignment.unwrap_or(false),
      object_guards: config.object_guards.unwrap_or(false),
      sourcemap: config.sourcemap.unwrap_or(false),
    }
  }
}
