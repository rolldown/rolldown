use napi_derive::napi;
use std::fmt::{self, Display, Formatter};

#[derive(Debug, PartialEq, Eq, Clone, Copy, Default)]
#[napi]
pub enum BindingLogLevel {
  Silent,
  Warn,
  #[default]
  Info,
  Debug,
}

impl From<String> for BindingLogLevel {
  fn from(value: String) -> Self {
    match value.as_str() {
      "silent" => Self::Silent,
      "warn" => Self::Warn,
      "info" => Self::Info,
      "debug" => Self::Debug,
      _ => panic!("Invalid log level: {value}"),
    }
  }
}

impl Display for BindingLogLevel {
  fn fmt(&self, f: &mut Formatter) -> fmt::Result {
    match self {
      Self::Silent => write!(f, "silent"),
      Self::Warn => write!(f, "warn"),
      Self::Info => write!(f, "info"),
      Self::Debug => write!(f, "debug"),
    }
  }
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
