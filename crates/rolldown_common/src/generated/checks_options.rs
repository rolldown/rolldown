// Auto-generated code, DO NOT EDIT DIRECTLY!
// To edit this generated file you have to edit `tasks/gen_options/src/generators/checks.rs`

#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::Deserialize;
#[derive(Default, Debug, Clone)]
#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(rename_all = "camelCase", deny_unknown_fields)
)]
pub struct ChecksOptions {
  pub circular_dependency: Option<bool>,
  pub eval: Option<bool>,
  pub missing_global_name: Option<bool>,
  pub missing_name_option_for_iife_export: Option<bool>,
  pub mixed_export: Option<bool>,
  pub unresolved_entry: Option<bool>,
  pub unresolved_import: Option<bool>,
  pub filename_conflict: Option<bool>,
  pub common_js_variable_in_esm: Option<bool>,
  pub import_is_undefined: Option<bool>,
}
