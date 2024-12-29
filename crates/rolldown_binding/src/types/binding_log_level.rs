use napi_derive::napi;
use std::fmt::{self, Display, Formatter};

#[derive(Debug, PartialEq, Clone, Copy, Default)]
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
