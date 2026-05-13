// Auto-generated code, DO NOT EDIT DIRECTLY!
// To edit this generated file you have to edit `tasks/generator/src/generators/checks.rs`

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug, Default)]
pub struct BindingChecksOptions {
  #[napi(ts_type = "false | 'warn' | 'error'")]
  pub circular_dependency: Option<napi::Either<bool, String>>,
  #[napi(ts_type = "false | 'warn' | 'error'")]
  pub eval: Option<napi::Either<bool, String>>,
  #[napi(ts_type = "false | 'warn' | 'error'")]
  pub missing_global_name: Option<napi::Either<bool, String>>,
  #[napi(ts_type = "false | 'warn' | 'error'")]
  pub missing_name_option_for_iife_export: Option<napi::Either<bool, String>>,
  #[napi(ts_type = "false | 'warn' | 'error'")]
  pub invalid_annotation: Option<napi::Either<bool, String>>,
  #[napi(ts_type = "false | 'warn' | 'error'")]
  pub mixed_exports: Option<napi::Either<bool, String>>,
  #[napi(ts_type = "false | 'warn' | 'error'")]
  pub unresolved_entry: Option<napi::Either<bool, String>>,
  #[napi(ts_type = "false | 'warn' | 'error'")]
  pub unresolved_import: Option<napi::Either<bool, String>>,
  #[napi(ts_type = "false | 'warn' | 'error'")]
  pub filename_conflict: Option<napi::Either<bool, String>>,
  #[napi(ts_type = "false | 'warn' | 'error'")]
  pub common_js_variable_in_esm: Option<napi::Either<bool, String>>,
  #[napi(ts_type = "false | 'warn' | 'error'")]
  pub import_is_undefined: Option<napi::Either<bool, String>>,
  #[napi(ts_type = "false | 'warn' | 'error'")]
  pub empty_import_meta: Option<napi::Either<bool, String>>,
  #[napi(ts_type = "false | 'warn' | 'error'")]
  pub tolerated_transform: Option<napi::Either<bool, String>>,
  #[napi(ts_type = "false | 'warn' | 'error'")]
  pub cannot_call_namespace: Option<napi::Either<bool, String>>,
  #[napi(ts_type = "false | 'warn' | 'error'")]
  pub configuration_field_conflict: Option<napi::Either<bool, String>>,
  #[napi(ts_type = "false | 'warn' | 'error'")]
  pub prefer_builtin_feature: Option<napi::Either<bool, String>>,
  #[napi(ts_type = "false | 'warn' | 'error'")]
  pub could_not_clean_directory: Option<napi::Either<bool, String>>,
  #[napi(ts_type = "false | 'warn' | 'error'")]
  pub plugin_timings: Option<napi::Either<bool, String>>,
  #[napi(ts_type = "false | 'warn' | 'error'")]
  pub duplicate_shebang: Option<napi::Either<bool, String>>,
  #[napi(ts_type = "false | 'warn' | 'error'")]
  pub unsupported_tsconfig_option: Option<napi::Either<bool, String>>,
  #[napi(ts_type = "false | 'warn' | 'error'")]
  pub ineffective_dynamic_import: Option<napi::Either<bool, String>>,
  #[napi(ts_type = "false | 'warn' | 'error'")]
  pub large_barrel_modules: Option<napi::Either<bool, String>>,
}
impl From<BindingChecksOptions> for rolldown_common::ChecksOptions {
  fn from(value: BindingChecksOptions) -> Self {
    Self {
      circular_dependency: value
        .circular_dependency
        .map(crate::utils::checks_severity::either_to_check_setting),
      eval: value.eval.map(crate::utils::checks_severity::either_to_check_setting),
      missing_global_name: value
        .missing_global_name
        .map(crate::utils::checks_severity::either_to_check_setting),
      missing_name_option_for_iife_export: value
        .missing_name_option_for_iife_export
        .map(crate::utils::checks_severity::either_to_check_setting),
      invalid_annotation: value
        .invalid_annotation
        .map(crate::utils::checks_severity::either_to_check_setting),
      mixed_exports: value
        .mixed_exports
        .map(crate::utils::checks_severity::either_to_check_setting),
      unresolved_entry: value
        .unresolved_entry
        .map(crate::utils::checks_severity::either_to_check_setting),
      unresolved_import: value
        .unresolved_import
        .map(crate::utils::checks_severity::either_to_check_setting),
      filename_conflict: value
        .filename_conflict
        .map(crate::utils::checks_severity::either_to_check_setting),
      common_js_variable_in_esm: value
        .common_js_variable_in_esm
        .map(crate::utils::checks_severity::either_to_check_setting),
      import_is_undefined: value
        .import_is_undefined
        .map(crate::utils::checks_severity::either_to_check_setting),
      empty_import_meta: value
        .empty_import_meta
        .map(crate::utils::checks_severity::either_to_check_setting),
      tolerated_transform: value
        .tolerated_transform
        .map(crate::utils::checks_severity::either_to_check_setting),
      cannot_call_namespace: value
        .cannot_call_namespace
        .map(crate::utils::checks_severity::either_to_check_setting),
      configuration_field_conflict: value
        .configuration_field_conflict
        .map(crate::utils::checks_severity::either_to_check_setting),
      prefer_builtin_feature: value
        .prefer_builtin_feature
        .map(crate::utils::checks_severity::either_to_check_setting),
      could_not_clean_directory: value
        .could_not_clean_directory
        .map(crate::utils::checks_severity::either_to_check_setting),
      plugin_timings: value
        .plugin_timings
        .map(crate::utils::checks_severity::either_to_check_setting),
      duplicate_shebang: value
        .duplicate_shebang
        .map(crate::utils::checks_severity::either_to_check_setting),
      unsupported_tsconfig_option: value
        .unsupported_tsconfig_option
        .map(crate::utils::checks_severity::either_to_check_setting),
      ineffective_dynamic_import: value
        .ineffective_dynamic_import
        .map(crate::utils::checks_severity::either_to_check_setting),
      large_barrel_modules: value
        .large_barrel_modules
        .map(crate::utils::checks_severity::either_to_check_setting),
    }
  }
}
