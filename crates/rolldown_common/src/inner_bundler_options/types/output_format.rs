#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::Deserialize;

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
