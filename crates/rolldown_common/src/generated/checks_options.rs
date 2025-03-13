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
impl From<ChecksOptions> for rolldown_error::EventKindSwitcher {
  fn from(value: ChecksOptions) -> Self {
    let mut flag = rolldown_error::EventKindSwitcher::all();
    flag.set(
      rolldown_error::EventKindSwitcher::CircularDependency,
      value.circular_dependency.unwrap_or(false),
    );
    flag.set(rolldown_error::EventKindSwitcher::Eval, value.eval.unwrap_or(true));
    flag.set(
      rolldown_error::EventKindSwitcher::MissingGlobalName,
      value.missing_global_name.unwrap_or(true),
    );
    flag.set(
      rolldown_error::EventKindSwitcher::MissingNameOptionForIifeExport,
      value.missing_name_option_for_iife_export.unwrap_or(true),
    );
    flag.set(rolldown_error::EventKindSwitcher::MixedExport, value.mixed_export.unwrap_or(true));
    flag.set(
      rolldown_error::EventKindSwitcher::UnresolvedEntry,
      value.unresolved_entry.unwrap_or(true),
    );
    flag.set(
      rolldown_error::EventKindSwitcher::UnresolvedImport,
      value.unresolved_import.unwrap_or(true),
    );
    flag.set(
      rolldown_error::EventKindSwitcher::FilenameConflict,
      value.filename_conflict.unwrap_or(true),
    );
    flag.set(
      rolldown_error::EventKindSwitcher::CommonJsVariableInEsm,
      value.common_js_variable_in_esm.unwrap_or(true),
    );
    flag.set(
      rolldown_error::EventKindSwitcher::ImportIsUndefined,
      value.import_is_undefined.unwrap_or(true),
    );
    flag
  }
}
