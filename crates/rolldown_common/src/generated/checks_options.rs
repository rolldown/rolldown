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
  pub circular_dependency: Option<crate::CheckSetting>,
  pub eval: Option<crate::CheckSetting>,
  pub missing_global_name: Option<crate::CheckSetting>,
  pub missing_name_option_for_iife_export: Option<crate::CheckSetting>,
  pub invalid_annotation: Option<crate::CheckSetting>,
  pub mixed_exports: Option<crate::CheckSetting>,
  pub unresolved_entry: Option<crate::CheckSetting>,
  pub unresolved_import: Option<crate::CheckSetting>,
  pub filename_conflict: Option<crate::CheckSetting>,
  pub common_js_variable_in_esm: Option<crate::CheckSetting>,
  pub import_is_undefined: Option<crate::CheckSetting>,
  pub empty_import_meta: Option<crate::CheckSetting>,
  pub tolerated_transform: Option<crate::CheckSetting>,
  pub cannot_call_namespace: Option<crate::CheckSetting>,
  pub configuration_field_conflict: Option<crate::CheckSetting>,
  pub prefer_builtin_feature: Option<crate::CheckSetting>,
  pub could_not_clean_directory: Option<crate::CheckSetting>,
  pub plugin_timings: Option<crate::CheckSetting>,
  pub duplicate_shebang: Option<crate::CheckSetting>,
  pub unsupported_tsconfig_option: Option<crate::CheckSetting>,
  pub ineffective_dynamic_import: Option<crate::CheckSetting>,
  pub large_barrel_modules: Option<crate::CheckSetting>,
}
/// Resolves the configured checks into two disjoint bitflags:
/// - `warn_checks`: kinds whose emissions fire at warning severity.
/// - `error_checks`: kinds whose emissions fire at hard-error severity.
///
/// Per kind, at most one flag is set (a check is either off, warn, or error).
/// `warn_checks` starts as `all()` so non-user-controllable kinds (errors, plugin
/// warnings) remain visible to `filter_out_disabled_diagnostics`. User-controllable
/// kinds are then explicitly placed in the right flag per the user's setting
/// (or the check's built-in default).
impl From<ChecksOptions>
  for (rolldown_error::EventKindSwitcher, rolldown_error::EventKindSwitcher)
{
  #[expect(clippy::too_many_lines)]
  fn from(value: ChecksOptions) -> Self {
    let mut warn_checks = rolldown_error::EventKindSwitcher::all();
    let mut error_checks = rolldown_error::EventKindSwitcher::empty();
    match value.circular_dependency {
      None | Some(crate::CheckSetting::Off) => {
        warn_checks.remove(rolldown_error::EventKindSwitcher::CircularDependency);
      }
      Some(crate::CheckSetting::Warn) => {
        warn_checks.insert(rolldown_error::EventKindSwitcher::CircularDependency);
      }
      Some(crate::CheckSetting::Error) => {
        warn_checks.remove(rolldown_error::EventKindSwitcher::CircularDependency);
        error_checks.insert(rolldown_error::EventKindSwitcher::CircularDependency);
      }
    }
    match value.eval {
      None => {}
      Some(crate::CheckSetting::Off) => {
        warn_checks.remove(rolldown_error::EventKindSwitcher::Eval);
      }
      Some(crate::CheckSetting::Warn) => {
        warn_checks.insert(rolldown_error::EventKindSwitcher::Eval);
      }
      Some(crate::CheckSetting::Error) => {
        warn_checks.remove(rolldown_error::EventKindSwitcher::Eval);
        error_checks.insert(rolldown_error::EventKindSwitcher::Eval);
      }
    }
    match value.missing_global_name {
      None => {}
      Some(crate::CheckSetting::Off) => {
        warn_checks.remove(rolldown_error::EventKindSwitcher::MissingGlobalName);
      }
      Some(crate::CheckSetting::Warn) => {
        warn_checks.insert(rolldown_error::EventKindSwitcher::MissingGlobalName);
      }
      Some(crate::CheckSetting::Error) => {
        warn_checks.remove(rolldown_error::EventKindSwitcher::MissingGlobalName);
        error_checks.insert(rolldown_error::EventKindSwitcher::MissingGlobalName);
      }
    }
    match value.missing_name_option_for_iife_export {
      None => {}
      Some(crate::CheckSetting::Off) => {
        warn_checks.remove(rolldown_error::EventKindSwitcher::MissingNameOptionForIifeExport);
      }
      Some(crate::CheckSetting::Warn) => {
        warn_checks.insert(rolldown_error::EventKindSwitcher::MissingNameOptionForIifeExport);
      }
      Some(crate::CheckSetting::Error) => {
        warn_checks.remove(rolldown_error::EventKindSwitcher::MissingNameOptionForIifeExport);
        error_checks.insert(rolldown_error::EventKindSwitcher::MissingNameOptionForIifeExport);
      }
    }
    match value.invalid_annotation {
      None => {}
      Some(crate::CheckSetting::Off) => {
        warn_checks.remove(rolldown_error::EventKindSwitcher::InvalidAnnotation);
      }
      Some(crate::CheckSetting::Warn) => {
        warn_checks.insert(rolldown_error::EventKindSwitcher::InvalidAnnotation);
      }
      Some(crate::CheckSetting::Error) => {
        warn_checks.remove(rolldown_error::EventKindSwitcher::InvalidAnnotation);
        error_checks.insert(rolldown_error::EventKindSwitcher::InvalidAnnotation);
      }
    }
    match value.mixed_exports {
      None => {}
      Some(crate::CheckSetting::Off) => {
        warn_checks.remove(rolldown_error::EventKindSwitcher::MixedExports);
      }
      Some(crate::CheckSetting::Warn) => {
        warn_checks.insert(rolldown_error::EventKindSwitcher::MixedExports);
      }
      Some(crate::CheckSetting::Error) => {
        warn_checks.remove(rolldown_error::EventKindSwitcher::MixedExports);
        error_checks.insert(rolldown_error::EventKindSwitcher::MixedExports);
      }
    }
    match value.unresolved_entry {
      None => {}
      Some(crate::CheckSetting::Off) => {
        warn_checks.remove(rolldown_error::EventKindSwitcher::UnresolvedEntry);
      }
      Some(crate::CheckSetting::Warn) => {
        warn_checks.insert(rolldown_error::EventKindSwitcher::UnresolvedEntry);
      }
      Some(crate::CheckSetting::Error) => {
        warn_checks.remove(rolldown_error::EventKindSwitcher::UnresolvedEntry);
        error_checks.insert(rolldown_error::EventKindSwitcher::UnresolvedEntry);
      }
    }
    match value.unresolved_import {
      None => {}
      Some(crate::CheckSetting::Off) => {
        warn_checks.remove(rolldown_error::EventKindSwitcher::UnresolvedImport);
      }
      Some(crate::CheckSetting::Warn) => {
        warn_checks.insert(rolldown_error::EventKindSwitcher::UnresolvedImport);
      }
      Some(crate::CheckSetting::Error) => {
        warn_checks.remove(rolldown_error::EventKindSwitcher::UnresolvedImport);
        error_checks.insert(rolldown_error::EventKindSwitcher::UnresolvedImport);
      }
    }
    match value.filename_conflict {
      None => {}
      Some(crate::CheckSetting::Off) => {
        warn_checks.remove(rolldown_error::EventKindSwitcher::FilenameConflict);
      }
      Some(crate::CheckSetting::Warn) => {
        warn_checks.insert(rolldown_error::EventKindSwitcher::FilenameConflict);
      }
      Some(crate::CheckSetting::Error) => {
        warn_checks.remove(rolldown_error::EventKindSwitcher::FilenameConflict);
        error_checks.insert(rolldown_error::EventKindSwitcher::FilenameConflict);
      }
    }
    match value.common_js_variable_in_esm {
      None => {}
      Some(crate::CheckSetting::Off) => {
        warn_checks.remove(rolldown_error::EventKindSwitcher::CommonJsVariableInEsm);
      }
      Some(crate::CheckSetting::Warn) => {
        warn_checks.insert(rolldown_error::EventKindSwitcher::CommonJsVariableInEsm);
      }
      Some(crate::CheckSetting::Error) => {
        warn_checks.remove(rolldown_error::EventKindSwitcher::CommonJsVariableInEsm);
        error_checks.insert(rolldown_error::EventKindSwitcher::CommonJsVariableInEsm);
      }
    }
    match value.import_is_undefined {
      None => {}
      Some(crate::CheckSetting::Off) => {
        warn_checks.remove(rolldown_error::EventKindSwitcher::ImportIsUndefined);
      }
      Some(crate::CheckSetting::Warn) => {
        warn_checks.insert(rolldown_error::EventKindSwitcher::ImportIsUndefined);
      }
      Some(crate::CheckSetting::Error) => {
        warn_checks.remove(rolldown_error::EventKindSwitcher::ImportIsUndefined);
        error_checks.insert(rolldown_error::EventKindSwitcher::ImportIsUndefined);
      }
    }
    match value.empty_import_meta {
      None => {}
      Some(crate::CheckSetting::Off) => {
        warn_checks.remove(rolldown_error::EventKindSwitcher::EmptyImportMeta);
      }
      Some(crate::CheckSetting::Warn) => {
        warn_checks.insert(rolldown_error::EventKindSwitcher::EmptyImportMeta);
      }
      Some(crate::CheckSetting::Error) => {
        warn_checks.remove(rolldown_error::EventKindSwitcher::EmptyImportMeta);
        error_checks.insert(rolldown_error::EventKindSwitcher::EmptyImportMeta);
      }
    }
    match value.tolerated_transform {
      None => {}
      Some(crate::CheckSetting::Off) => {
        warn_checks.remove(rolldown_error::EventKindSwitcher::ToleratedTransform);
      }
      Some(crate::CheckSetting::Warn) => {
        warn_checks.insert(rolldown_error::EventKindSwitcher::ToleratedTransform);
      }
      Some(crate::CheckSetting::Error) => {
        warn_checks.remove(rolldown_error::EventKindSwitcher::ToleratedTransform);
        error_checks.insert(rolldown_error::EventKindSwitcher::ToleratedTransform);
      }
    }
    match value.cannot_call_namespace {
      None => {}
      Some(crate::CheckSetting::Off) => {
        warn_checks.remove(rolldown_error::EventKindSwitcher::CannotCallNamespace);
      }
      Some(crate::CheckSetting::Warn) => {
        warn_checks.insert(rolldown_error::EventKindSwitcher::CannotCallNamespace);
      }
      Some(crate::CheckSetting::Error) => {
        warn_checks.remove(rolldown_error::EventKindSwitcher::CannotCallNamespace);
        error_checks.insert(rolldown_error::EventKindSwitcher::CannotCallNamespace);
      }
    }
    match value.configuration_field_conflict {
      None => {}
      Some(crate::CheckSetting::Off) => {
        warn_checks.remove(rolldown_error::EventKindSwitcher::ConfigurationFieldConflict);
      }
      Some(crate::CheckSetting::Warn) => {
        warn_checks.insert(rolldown_error::EventKindSwitcher::ConfigurationFieldConflict);
      }
      Some(crate::CheckSetting::Error) => {
        warn_checks.remove(rolldown_error::EventKindSwitcher::ConfigurationFieldConflict);
        error_checks.insert(rolldown_error::EventKindSwitcher::ConfigurationFieldConflict);
      }
    }
    match value.prefer_builtin_feature {
      None => {}
      Some(crate::CheckSetting::Off) => {
        warn_checks.remove(rolldown_error::EventKindSwitcher::PreferBuiltinFeature);
      }
      Some(crate::CheckSetting::Warn) => {
        warn_checks.insert(rolldown_error::EventKindSwitcher::PreferBuiltinFeature);
      }
      Some(crate::CheckSetting::Error) => {
        warn_checks.remove(rolldown_error::EventKindSwitcher::PreferBuiltinFeature);
        error_checks.insert(rolldown_error::EventKindSwitcher::PreferBuiltinFeature);
      }
    }
    match value.could_not_clean_directory {
      None => {}
      Some(crate::CheckSetting::Off) => {
        warn_checks.remove(rolldown_error::EventKindSwitcher::CouldNotCleanDirectory);
      }
      Some(crate::CheckSetting::Warn) => {
        warn_checks.insert(rolldown_error::EventKindSwitcher::CouldNotCleanDirectory);
      }
      Some(crate::CheckSetting::Error) => {
        warn_checks.remove(rolldown_error::EventKindSwitcher::CouldNotCleanDirectory);
        error_checks.insert(rolldown_error::EventKindSwitcher::CouldNotCleanDirectory);
      }
    }
    match value.plugin_timings {
      None => {}
      Some(crate::CheckSetting::Off) => {
        warn_checks.remove(rolldown_error::EventKindSwitcher::PluginTimings);
      }
      Some(crate::CheckSetting::Warn) => {
        warn_checks.insert(rolldown_error::EventKindSwitcher::PluginTimings);
      }
      Some(crate::CheckSetting::Error) => {
        warn_checks.remove(rolldown_error::EventKindSwitcher::PluginTimings);
        error_checks.insert(rolldown_error::EventKindSwitcher::PluginTimings);
      }
    }
    match value.duplicate_shebang {
      None => {}
      Some(crate::CheckSetting::Off) => {
        warn_checks.remove(rolldown_error::EventKindSwitcher::DuplicateShebang);
      }
      Some(crate::CheckSetting::Warn) => {
        warn_checks.insert(rolldown_error::EventKindSwitcher::DuplicateShebang);
      }
      Some(crate::CheckSetting::Error) => {
        warn_checks.remove(rolldown_error::EventKindSwitcher::DuplicateShebang);
        error_checks.insert(rolldown_error::EventKindSwitcher::DuplicateShebang);
      }
    }
    match value.unsupported_tsconfig_option {
      None => {}
      Some(crate::CheckSetting::Off) => {
        warn_checks.remove(rolldown_error::EventKindSwitcher::UnsupportedTsconfigOption);
      }
      Some(crate::CheckSetting::Warn) => {
        warn_checks.insert(rolldown_error::EventKindSwitcher::UnsupportedTsconfigOption);
      }
      Some(crate::CheckSetting::Error) => {
        warn_checks.remove(rolldown_error::EventKindSwitcher::UnsupportedTsconfigOption);
        error_checks.insert(rolldown_error::EventKindSwitcher::UnsupportedTsconfigOption);
      }
    }
    match value.ineffective_dynamic_import {
      None => {}
      Some(crate::CheckSetting::Off) => {
        warn_checks.remove(rolldown_error::EventKindSwitcher::IneffectiveDynamicImport);
      }
      Some(crate::CheckSetting::Warn) => {
        warn_checks.insert(rolldown_error::EventKindSwitcher::IneffectiveDynamicImport);
      }
      Some(crate::CheckSetting::Error) => {
        warn_checks.remove(rolldown_error::EventKindSwitcher::IneffectiveDynamicImport);
        error_checks.insert(rolldown_error::EventKindSwitcher::IneffectiveDynamicImport);
      }
    }
    match value.large_barrel_modules {
      None => {}
      Some(crate::CheckSetting::Off) => {
        warn_checks.remove(rolldown_error::EventKindSwitcher::LargeBarrelModules);
      }
      Some(crate::CheckSetting::Warn) => {
        warn_checks.insert(rolldown_error::EventKindSwitcher::LargeBarrelModules);
      }
      Some(crate::CheckSetting::Error) => {
        warn_checks.remove(rolldown_error::EventKindSwitcher::LargeBarrelModules);
        error_checks.insert(rolldown_error::EventKindSwitcher::LargeBarrelModules);
      }
    }
    (warn_checks, error_checks)
  }
}
