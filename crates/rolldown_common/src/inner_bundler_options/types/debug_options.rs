#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::Deserialize;

#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(rename_all = "camelCase", deny_unknown_fields)
)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugOptions {
  /// If not set, a random id will be generated.
  pub build_id: Option<String>,
  /// If set, `<OUT>/debug.db` will be created in the output directory.
  pub db_addr: Option<String>,
}
