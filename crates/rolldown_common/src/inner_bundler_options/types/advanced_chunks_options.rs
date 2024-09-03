#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::Deserialize;

#[derive(Default, Debug)]
#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(rename_all = "camelCase", deny_unknown_fields)
)]
pub struct AdvancedChunksOptions {
  // pub share_count: Option<u32>,
  pub groups: Option<Vec<MatchGroup>>,
}

#[derive(Default, Debug)]
#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(rename_all = "camelCase", deny_unknown_fields)
)]
pub struct MatchGroup {
  pub name: String,
  pub test: Option<String>,
  // pub share_count: Option<u32>,
  pub priority: Option<u32>,
}
