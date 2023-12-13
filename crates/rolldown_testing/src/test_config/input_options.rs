use schemars::JsonSchema;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InputOptions {
  pub input: Option<Vec<InputItem>>,
  pub external: Option<Vec<String>>,
  pub treeshake: Option<bool>,
  pub resolve: Option<ResolveOptions>,
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

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResolveOptions {
  pub alias: Option<HashMap<String, Vec<String>>>,
  pub alias_fields: Option<Vec<Vec<String>>>,
  pub condition_names: Option<Vec<String>>,
  pub exports_fields: Option<Vec<Vec<String>>>,
  pub extensions: Option<Vec<String>>,
  pub main_fields: Option<Vec<String>>,
  pub main_files: Option<Vec<String>>,
  pub modules: Option<Vec<String>>,
  pub symlinks: Option<bool>,
}
