use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DevTestMeta {
  // Empty for now - will be used to control test behavior in the dev scenario
}

impl Default for DevTestMeta {
  fn default() -> Self {
    serde_json::from_str("{}").unwrap()
  }
}
