#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug, Default)]
pub struct BindingDevtoolsOptions {
  pub session_id: Option<String>,
}
