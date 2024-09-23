#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::Deserialize;

#[derive(Debug, Default)]
#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(rename_all = "camelCase", deny_unknown_fields)
)]
pub struct ExperimentalOptions {
  pub strict_execution_order: Option<bool>,
  pub disable_live_bindings: Option<bool>,
}

impl ExperimentalOptions {
  pub fn is_strict_execution_order_enabled(&self) -> bool {
    self.strict_execution_order.unwrap_or(false)
  }

  pub fn is_disable_live_bindings_enabled(&self) -> bool {
    self.disable_live_bindings.unwrap_or(false)
  }
}
