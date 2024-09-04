use rolldown_utils::js_regex::HybridRegex;
#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::{Deserialize, Deserializer};

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
  #[cfg_attr(
    feature = "deserialize_bundler_options",
    serde(deserialize_with = "deserialize_test", default),
    schemars(with = "Option<String>")
  )]
  pub test: Option<HybridRegex>,
  // pub share_count: Option<u32>,
  pub priority: Option<u32>,
}

#[cfg(feature = "deserialize_bundler_options")]
fn deserialize_test<'de, D>(deserializer: D) -> Result<Option<HybridRegex>, D::Error>
where
  D: Deserializer<'de>,
{
  let deserialized = Option::<String>::deserialize(deserializer)?;
  let transformed = deserialized
    .map(|inner| HybridRegex::new(&inner))
    .transpose()
    .map_err(|e| serde::de::Error::custom(format!("failed to deserialize {e:?} to HybridRegex")))?;
  Ok(transformed)
}
