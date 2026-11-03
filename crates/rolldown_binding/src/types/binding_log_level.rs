use napi_derive::napi;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Default)]
#[napi]
pub enum BindingLogLevel {
  Silent,
  Warn,
  #[default]
  Info,
  Debug,
}

impl From<BindingLogLevel> for rolldown_common::LogLevel {
  fn from(value: BindingLogLevel) -> Self {
    match value {
      BindingLogLevel::Silent => Self::Silent,
      BindingLogLevel::Warn => Self::Warn,
      BindingLogLevel::Info => Self::Info,
      BindingLogLevel::Debug => Self::Debug,
    }
  }
}
