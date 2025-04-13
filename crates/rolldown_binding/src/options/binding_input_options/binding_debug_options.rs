#[napi_derive::napi(object)]
#[derive(Debug, Default)]
pub struct BindingDebugOptions {
  pub build_id: Option<String>,
}
