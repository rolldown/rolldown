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
  Auto(bool),
  Manual(PathBuf),
}

impl Default for TsConfig {
  fn default() -> Self {
    Self::Auto(true)
  }
}

impl TsConfig {
  #[must_use]
  pub fn with_base(self, base: &Path) -> Self {
    match self {
      Self::Auto(_) => self,
      Self::Manual(path) => Self::Manual(base.join(path).normalize().into_owned()),
    }
  }
}
