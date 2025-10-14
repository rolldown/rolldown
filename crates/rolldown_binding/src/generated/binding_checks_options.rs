// Auto-generated code, DO NOT EDIT DIRECTLY!
// To edit this generated file you have to edit `tasks/generator/src/generators/checks.rs`

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug, Default)]
pub struct BindingChecksOptions {
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
  pub empty_import_meta: Option<bool>,
  pub configuration_field_conflict: Option<bool>,
  pub prefer_builtin_feature: Option<bool>,
}
impl From<BindingChecksOptions> for rolldown_common::ChecksOptions {
  fn from(value: BindingChecksOptions) -> Self {
    Self {
      circular_dependency: value.circular_dependency,
      eval: value.eval,
      missing_global_name: value.missing_global_name,
      missing_name_option_for_iife_export: value.missing_name_option_for_iife_export,
      mixed_export: value.mixed_export,
      unresolved_entry: value.unresolved_entry,
      unresolved_import: value.unresolved_import,
      filename_conflict: value.filename_conflict,
      common_js_variable_in_esm: value.common_js_variable_in_esm,
      import_is_undefined: value.import_is_undefined,
      empty_import_meta: value.empty_import_meta,
      configuration_field_conflict: value.configuration_field_conflict,
      prefer_builtin_feature: value.prefer_builtin_feature,
    }
  }
}
