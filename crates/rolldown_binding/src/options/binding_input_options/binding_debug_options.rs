#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug, Default)]
pub struct BindingDebugOptions {
  pub session_id: Option<String>,
}
