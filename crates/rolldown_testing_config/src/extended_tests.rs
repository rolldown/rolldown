use schemars::JsonSchema;
use serde::Deserialize;

use crate::utils::true_by_default;

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExtendedTests {
  /// Run the test case with `minifyInternalExports` enabled in addition to the default config.
  #[serde(default = "true_by_default")]
  pub minify_internal_exports: bool,
  /// Run the test case with `minify` enabled in addition to the default config.
  #[serde(default)]
  pub minify: bool,
}

impl Default for ExtendedTests {
  fn default() -> Self {
    serde_json::from_str("{}").unwrap()
  }
}
