use super::binding_import_attributes::BindingImportAttributes;

/// Only passed for a module with import attributes.
#[napi_derive::napi(object, object_from_js = false)]
#[derive(Default, Debug)]
pub struct BindingHookLoadExtraArgs {
  /// The module id without its import attributes.
  pub raw_id: String,
  #[napi(ts_type = "Record<string, string>")]
  pub attributes: BindingImportAttributes,
}
