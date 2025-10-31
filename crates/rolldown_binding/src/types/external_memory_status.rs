use napi_derive::napi;

#[napi(object, object_from_js = false)]
pub struct ExternalMemoryStatus {
  pub freed: bool,
  pub reason: Option<String>,
}
