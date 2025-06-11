#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::Deserialize;

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "deserialize_bundler_options", derive(Deserialize, JsonSchema))]
#[cfg_attr(feature = "deserialize_bundler_options", serde(rename_all = "camelCase"))]
pub enum AttachDebugInfo {
  None,
  Simple,
  Full,
}

impl AttachDebugInfo {
  pub fn is_enabled(&self) -> bool {
    !matches!(self, AttachDebugInfo::None)
  }

  pub fn is_full(&self) -> bool {
    matches!(self, AttachDebugInfo::Full)
  }
}
