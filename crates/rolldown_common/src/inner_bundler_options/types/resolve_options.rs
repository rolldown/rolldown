use schemars::JsonSchema;
use serde::Deserialize;

/// A simple wrapper around `oxc_resolver::ResolveOptions` to make it easier to use in the `rolldown_resolver` crate.
/// See [oxc_resolver::ResolveOptions](https://docs.rs/oxc_resolver/latest/oxc_resolver/struct.ResolveOptions.html) for more information.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResolveOptions {
  pub alias: Option<Vec<(String, Vec<String>)>>,
  pub alias_fields: Option<Vec<Vec<String>>>,
  pub condition_names: Option<Vec<String>>,
  pub exports_fields: Option<Vec<Vec<String>>>,
  pub extensions: Option<Vec<String>>,
  pub main_fields: Option<Vec<String>>,
  pub main_files: Option<Vec<String>>,
  pub modules: Option<Vec<String>>,
  pub symlinks: Option<bool>,
  pub tsconfig_filename: Option<String>,
}
