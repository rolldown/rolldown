use std::{pin::Pin, sync::Arc};

use rolldown_utils::js_regex::HybridRegex;
#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::{Deserialize, Deserializer};

#[derive(Default, Debug, Clone)]
#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(rename_all = "camelCase", deny_unknown_fields)
)]
pub struct AdvancedChunksOptions {
  pub min_share_count: Option<u32>,
  pub min_size: Option<f64>,
  pub max_size: Option<f64>,
  pub min_module_size: Option<f64>,
  pub max_module_size: Option<f64>,
  /// Whether to include the captured module's dependencies recursively.
  /// - If `true`, the dependencies would be included this group forcefully unless they are already included in another group.
  /// - This option would forcefully `true`, if `preserve_entry_signatures` is not `allow-extension`.
  pub include_dependencies_recursively: Option<bool>,
  pub groups: Option<Vec<MatchGroup>>,
}

#[derive(Default, Debug, Clone)]
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
  pub test: Option<MatchGroupTest>,
  // pub share_count: Option<u32>,
  pub priority: Option<u32>,
  pub min_size: Option<f64>,
  pub max_size: Option<f64>,
  pub min_share_count: Option<u32>,
  pub min_module_size: Option<f64>,
  pub max_module_size: Option<f64>,
}

type MatchGroupTestFn = dyn Fn(&str) -> Pin<Box<(dyn Future<Output = anyhow::Result<Option<bool>>> + Send + 'static)>>
  + Send
  + Sync;

#[derive(derive_more::Debug, Clone)]
pub enum MatchGroupTest {
  Regex(HybridRegex),
  #[debug("Function")]
  Function(Arc<MatchGroupTestFn>),
}

#[cfg(feature = "deserialize_bundler_options")]
fn deserialize_test<'de, D>(deserializer: D) -> Result<Option<MatchGroupTest>, D::Error>
where
  D: Deserializer<'de>,
{
  let deserialized = Option::<String>::deserialize(deserializer)?;
  let transformed = deserialized
    .map(|inner| HybridRegex::new(&inner))
    .transpose()
    .map_err(|e| serde::de::Error::custom(format!("failed to deserialize {e:?} to HybridRegex")))?;
  Ok(transformed.map(MatchGroupTest::Regex))
}
