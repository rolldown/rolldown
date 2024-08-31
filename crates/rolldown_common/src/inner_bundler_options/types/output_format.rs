#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::Deserialize;
use std::fmt::Display;

#[derive(Debug)]
#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(rename_all = "camelCase", deny_unknown_fields)
)]
pub enum OutputFormat {
  Esm,
  Cjs,
  App,
  Iife,
}

impl OutputFormat {
  pub fn requires_scope_hoisting(&self) -> bool {
    matches!(self, Self::Esm | Self::Cjs | Self::Iife)
  }
}

impl Display for OutputFormat {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Esm => write!(f, "esm"),
      Self::Cjs => write!(f, "cjs"),
      Self::App => write!(f, "app"),
      Self::Iife => write!(f, "iife"),
    }
  }
}
