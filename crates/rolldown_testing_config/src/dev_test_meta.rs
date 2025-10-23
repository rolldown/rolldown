use rolldown_dev_common::types::DevOptions;
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DevTestMeta {
  #[serde(default)]
  pub config: Option<DevOptions>,
}

impl Default for DevTestMeta {
  fn default() -> Self {
    serde_json::from_str("{}").unwrap()
  }
}
