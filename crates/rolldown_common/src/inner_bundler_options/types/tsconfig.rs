use std::path::{Path, PathBuf};

#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::{Deserialize, Deserializer};
use sugar_path::SugarPath as _;

#[derive(Debug, Clone)]
#[cfg_attr(feature = "deserialize_bundler_options", derive(Deserialize, JsonSchema))]
#[cfg_attr(feature = "deserialize_bundler_options", serde(untagged))]
pub enum TsConfig {
  #[cfg_attr(
    feature = "deserialize_bundler_options",
    serde(deserialize_with = "deserialize_tsconfig_auto")
  )]
  #[cfg_attr(
    feature = "deserialize_bundler_options",
    schemars(schema_with = "schemars_for_tsconfig_auto")
  )]
  Auto,
  Manual(PathBuf),
}

impl TsConfig {
  #[must_use]
  pub fn with_base(self, base: &Path) -> Self {
    match self {
      Self::Auto => self,
      Self::Manual(path) => Self::Manual(base.join(path).normalize()),
    }
  }
}

#[cfg(feature = "deserialize_bundler_options")]
fn deserialize_tsconfig_auto<'de, D>(deserializer: D) -> Result<(), D::Error>
where
  D: Deserializer<'de>,
{
  let deserialized = bool::deserialize(deserializer)?;
  if deserialized { Ok(()) } else { Err(serde::de::Error::custom("expected true or path")) }
}

#[cfg(feature = "deserialize_bundler_options")]
fn schemars_for_tsconfig_auto(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
  schemars::json_schema!({ "type": "boolean", "const": true })
}
