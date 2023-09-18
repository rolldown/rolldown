use schemars::JsonSchema;
use serde::Deserialize;

use crate::impl_serde_default;

fn esm_by_default() -> String {
  "esm".to_string()
}

fn auto_by_default() -> String {
  "auto".to_string()
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OutputOptions {
  #[serde(default = "esm_by_default")]
  pub format: String,
  #[serde(default = "auto_by_default")]
  pub export_mode: String,
}

impl_serde_default!(OutputOptions);
