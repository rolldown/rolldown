use std::fmt;

#[cfg(feature = "deserialize_dev_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_dev_options")]
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "deserialize_dev_options", derive(Deserialize, JsonSchema))]
#[cfg_attr(feature = "deserialize_dev_options", serde(rename_all = "camelCase"))]
pub enum RebuildStrategy {
  /// Incremental rebuild will always be issued after HMR.
  Always,
  /// Incremental rebuild will be issued automatically if the hmr updates contains full reload updates.
  #[default]
  Auto,
  /// Never issue rebuilds after HMR.
  Never,
}

impl RebuildStrategy {
  pub fn is_always(&self) -> bool {
    matches!(self, Self::Always)
  }

  pub fn is_auto(&self) -> bool {
    matches!(self, Self::Auto)
  }
}

impl fmt::Display for RebuildStrategy {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Always => write!(f, "always"),
      Self::Auto => write!(f, "auto"),
      Self::Never => write!(f, "never"),
    }
  }
}
