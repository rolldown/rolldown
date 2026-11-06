use super::binding_import_attributes::BindingImportAttributes;

#[napi_derive::napi(object, object_from_js = false)]
#[derive(Default, Debug)]
pub struct BindingHookLoadExtraArgs {
  pub raw_id: String,
  #[napi(ts_type = "Record<string, string>")]
  pub attributes: BindingImportAttributes,
}
