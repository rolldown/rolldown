use schemars::JsonSchema;
use serde::Deserialize;

use crate::utils::true_by_default;

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExtendedTests {
  /// Run the test case with the opposite value of `minifyInternalExports` compared to what the default would be.
  /// If it's explicitly set in the config, this option has no effect.
  /// If the default resolves to `true` (e.g., format: 'es' or minify: true), tests with `false`.
  /// If the default resolves to `false` (e.g., format: 'cjs' without minify), tests with `true`.
  #[serde(default = "true_by_default")]
  pub opposite_minify_internal_exports: bool,
  /// Run the test case with `preserveEntrySignatures: 'strict'` in addition to the default.
  /// If `preserveEntrySignatures` is explicitly set in the config, this option has no effect.
  #[serde(default = "true_by_default")]
  pub preserve_entry_signatures_strict: bool,
  /// Run the test case with `preserveEntrySignatures: 'allow-extension'` in addition to the default.
  /// If `preserveEntrySignatures` is explicitly set in the config, this option has no effect.
  #[serde(default = "true_by_default")]
  pub preserve_entry_signatures_allow_extension: bool,
}

impl Default for ExtendedTests {
  fn default() -> Self {
    serde_json::from_str("{}").unwrap()
  }
}
