use napi_derive::napi;

#[napi(object)]
pub struct BindingLog {
  pub code: String,
  pub message: String,
  pub id: Option<String>,
  pub exporter: Option<String>,
}
