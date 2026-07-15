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
  /// Never issue rebuilds after HMR. The server no longer decides full reloads, so
  /// there is no `auto` upgrade anymore; consumers that want fresh bundle output pull
  /// it explicitly (e.g. `ensure_latest_bundle_output`).
  #[default]
  Never,
}

impl RebuildStrategy {
  pub fn is_always(&self) -> bool {
    matches!(self, Self::Always)
  }
}

impl fmt::Display for RebuildStrategy {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Always => write!(f, "always"),
      Self::Never => write!(f, "never"),
    }
  }
}
