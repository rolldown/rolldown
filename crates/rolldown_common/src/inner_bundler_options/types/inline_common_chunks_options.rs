#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::Deserialize;

use super::manual_code_splitting_options::MatchGroupTest;

/// `output.codeSplitting.experimentalInlineCommonChunks`, as the user wrote it.
///
/// See internal-docs/inline-common-chunks/design.md.
#[derive(Debug, Clone)]
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
  #[cfg_attr(feature = "deserialize_bundler_options", serde(skip), schemars(skip))]
  pub exclude: Option<Vec<MatchGroupTest>>,
}

/// The validated form the bundler reads. `NormalizedBundlerOptions::inline_common_chunks` is
/// `Some` exactly when the feature is on: `maxSize: 0` normalizes to `None`, so no reader can tell
/// it from an absent option.
#[derive(Debug, Clone)]
pub struct NormalizedInlineCommonChunksOptions {
  /// Source bytes; always above zero.
  pub max_size: usize,
  pub exclude: Vec<MatchGroupTest>,
}
