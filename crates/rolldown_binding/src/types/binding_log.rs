use napi_derive::napi;

#[napi]
pub struct BindingLog {
  pub code: String,
  pub message: String,
}
