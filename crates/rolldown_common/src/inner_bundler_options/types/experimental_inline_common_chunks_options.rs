#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::Deserialize;

/// Experimental policy for replacing small automatic common chunks with factory definitions in
/// their consumers. The threshold uses module source bytes because rendered chunk sizes are not
/// available when placement is decided.
#[derive(Default, Debug, Clone, Copy)]
#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(rename_all = "camelCase", deny_unknown_fields)
)]
pub struct ExperimentalInlineCommonChunksOptions {
  /// Common chunks whose transformed module source size is below this byte threshold may be
  /// inlined. Zero or an omitted value disables the feature.
  pub max_size: Option<f64>,
}
