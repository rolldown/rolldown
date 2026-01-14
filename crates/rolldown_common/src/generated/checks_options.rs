// Auto-generated code, DO NOT EDIT DIRECTLY!
// To edit this generated file you have to edit `tasks/generator/src/generators/checks.rs`

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
  pub mixed_exports: Option<bool>,
  pub unresolved_entry: Option<bool>,
  pub unresolved_import: Option<bool>,
  pub filename_conflict: Option<bool>,
  pub common_js_variable_in_esm: Option<bool>,
  pub import_is_undefined: Option<bool>,
  pub empty_import_meta: Option<bool>,
  pub tolerated_transform: Option<bool>,
  pub cannot_call_namespace: Option<bool>,
  pub configuration_field_conflict: Option<bool>,
  pub prefer_builtin_feature: Option<bool>,
  pub could_not_clean_directory: Option<bool>,
  pub plugin_timings: Option<bool>,
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
    flag.set(rolldown_error::EventKindSwitcher::MixedExports, value.mixed_exports.unwrap_or(true));
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
    flag.set(
      rolldown_error::EventKindSwitcher::EmptyImportMeta,
      value.empty_import_meta.unwrap_or(true),
    );
    flag.set(
      rolldown_error::EventKindSwitcher::ToleratedTransform,
      value.tolerated_transform.unwrap_or(true),
    );
    flag.set(
      rolldown_error::EventKindSwitcher::CannotCallNamespace,
      value.cannot_call_namespace.unwrap_or(true),
    );
    flag.set(
      rolldown_error::EventKindSwitcher::ConfigurationFieldConflict,
      value.configuration_field_conflict.unwrap_or(true),
    );
    flag.set(
      rolldown_error::EventKindSwitcher::PreferBuiltinFeature,
      value.prefer_builtin_feature.unwrap_or(true),
    );
    flag.set(
      rolldown_error::EventKindSwitcher::CouldNotCleanDirectory,
      value.could_not_clean_directory.unwrap_or(true),
    );
    flag
      .set(rolldown_error::EventKindSwitcher::PluginTimings, value.plugin_timings.unwrap_or(true));
    flag
  }
}
