use napi_derive::napi;

#[derive(Debug, PartialEq)]
#[napi]
pub enum LogLevel {
  Silent,
  Warn,
  Info,
  Debug,
}

impl From<String> for LogLevel {
  fn from(value: String) -> Self {
    match value.as_str() {
      "silent" => Self::Silent,
      "warn" => Self::Warn,
      "info" => Self::Info,
      "debug" => Self::Debug,
      _ => panic!("Invalid log level: {}", value),
    }
  }
}
