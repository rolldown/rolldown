use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PluginTestMeta {
  #[serde(default)]
  pub css: Option<CssPluginTestConfig>,
}

#[derive(Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CssPluginTestConfig {
  #[serde(default)]
  pub code_split: Option<bool>,
  #[serde(default)]
  pub minify: Option<bool>,
  #[serde(default)]
  pub sourcemap: Option<bool>,
}
