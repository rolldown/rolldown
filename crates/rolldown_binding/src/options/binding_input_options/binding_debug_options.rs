#[napi_derive::napi(object)]
#[derive(Debug, Default)]
pub struct BindingDebugOptions {
  pub session_id: Option<String>,
}
