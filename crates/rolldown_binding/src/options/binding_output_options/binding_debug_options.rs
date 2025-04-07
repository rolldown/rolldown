#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug)]
pub struct BindingDebugOptions {
  pub build_id: Option<String>,
  pub db_addr: Option<String>,
}
