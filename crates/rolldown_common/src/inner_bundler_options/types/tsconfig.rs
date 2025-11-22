use std::path::{Path, PathBuf};

#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::Deserialize;
use sugar_path::SugarPath as _;

#[derive(Debug, Clone)]
#[cfg_attr(feature = "deserialize_bundler_options", derive(Deserialize, JsonSchema))]
#[cfg_attr(feature = "deserialize_bundler_options", serde(untagged))]
pub enum TsConfig {
  Auto,
  Special(PathBuf),
}

impl TsConfig {
  #[must_use]
  pub fn with_base(self, base: &Path) -> Self {
    match self {
      Self::Auto => self,
      Self::Special(path) => Self::Special(base.join(path).normalize()),
    }
  }
}
