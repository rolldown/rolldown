use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InputOptions {
  pub input: Option<Vec<InputItem>>,
  pub external: Option<Vec<String>>,
  pub treeshake: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InputItem {
  pub name: String,
  pub import: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TsConfig {
  #[serde(default)]
  pub use_define_for_class_fields: bool,
}
