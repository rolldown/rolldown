#[cfg(feature = "deserialize_bundler_options")]
use rolldown_utils::js_regex::HybridRegex;
#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::{Deserialize, Deserializer};

use super::manual_code_splitting_options::MatchGroupTest;

/// `output.codeSplitting.experimentalInlineCommonChunks`, as the user wrote it.
///
/// See internal-docs/inline-common-chunks/design.md.
#[derive(Default, Debug, Clone)]
#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(rename_all = "camelCase", deny_unknown_fields)
)]
pub struct InlineCommonChunksOptions {
  /// A candidate's pre-render size must be strictly smaller than this. `0` (the default) turns
  /// the feature off.
  pub max_size: Option<f64>,
  /// Module id matchers with the same rules as `CodeSplittingGroup.test`. A candidate is kept as
  /// a file when any of its modules matches.
  #[cfg_attr(
    feature = "deserialize_bundler_options",
    serde(deserialize_with = "deserialize_exclude", default),
    schemars(with = "Option<Vec<String>>")
  )]
  pub exclude: Option<Vec<MatchGroupTest>>,
}

#[cfg(feature = "deserialize_bundler_options")]
fn deserialize_exclude<'de, D>(deserializer: D) -> Result<Option<Vec<MatchGroupTest>>, D::Error>
where
  D: Deserializer<'de>,
{
  let deserialized = Option::<Vec<String>>::deserialize(deserializer)?;
  deserialized
    .map(|patterns| {
      patterns
        .iter()
        .map(|pattern| {
          HybridRegex::new(pattern).map(MatchGroupTest::Regex).map_err(|e| {
            serde::de::Error::custom(format!("failed to deserialize {e} to HybridRegex"))
          })
        })
        .collect::<Result<Vec<_>, _>>()
    })
    .transpose()
}

/// The validated form the bundler reads. `max_size == 0` means the feature is off and every
/// reader must behave exactly as if the option were absent.
#[derive(Debug, Clone)]
pub struct NormalizedInlineCommonChunksOptions {
  pub max_size: u64,
  pub exclude: Vec<MatchGroupTest>,
}

impl NormalizedInlineCommonChunksOptions {
  pub fn is_enabled(&self) -> bool {
    self.max_size > 0
  }
}
