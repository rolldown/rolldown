#[napi_derive::napi(object)]
pub struct BindingLog {
  pub message: String,
  pub id: Option<String>,
  pub code: Option<String>,
  pub exporter: Option<String>,
  pub plugin: Option<String>,
}

impl From<rolldown_common::Log> for BindingLog {
  fn from(value: rolldown_common::Log) -> Self {
    Self {
      code: value.code,
      message: value.message,
      id: value.id,
      exporter: value.exporter,
      plugin: value.plugin,
    }
  }
}
