#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::Deserialize;
use std::fmt::{self, Display, Formatter};

#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(rename_all = "camelCase", deny_unknown_fields)
)]
#[derive(Debug, PartialEq, Eq, Clone, Copy, Default)]
pub enum LogLevel {
  Silent,
  Warn,
  #[default]
  Info,
  Debug,
}

impl Display for LogLevel {
  fn fmt(&self, f: &mut Formatter) -> fmt::Result {
    match self {
      Self::Silent => write!(f, "silent"),
      Self::Warn => write!(f, "warn"),
      Self::Info => write!(f, "info"),
      Self::Debug => write!(f, "debug"),
    }
  }
}
