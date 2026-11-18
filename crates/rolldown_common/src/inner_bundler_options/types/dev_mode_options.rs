#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::Deserialize;

#[derive(Debug, Default, Clone)]
#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(rename_all = "camelCase", deny_unknown_fields)
)]
pub struct DevModeOptions {
  /// Custom dev mode runtime implementation.
  pub implement: Option<String>,
  /// Enable lazy compilation for dynamic imports.
  pub lazy: Option<bool>,
  /// Call the `hotUpdate` plugin hook. Off by default. The `hotUpdate` dev option also turns it on.
  pub hot_update: Option<bool>,
}
