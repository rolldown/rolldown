use napi_derive::napi;

#[napi(object)]
pub struct BindingLog {
  pub code: String,
  pub message: String,
  pub id: Option<String>,
  pub exporter: Option<String>,
}

impl From<rolldown_common::Log> for BindingLog {
  fn from(value: rolldown_common::Log) -> Self {
    Self { code: value.code, message: value.message, id: value.id, exporter: value.exporter }
  }
}
