use std::fmt::Display;

#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::Deserialize;

/// Controls how code splitting is performed.
///
/// - `Bool(true)`: Default behavior, automatic code splitting with lazy-loaded dynamic imports.
/// - `Bool(false)`: Inline all dynamic imports into a single bundle (no code splitting).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(untagged)
)]
pub enum CodeSplittingMode {
  Bool(bool),
}

impl Default for CodeSplittingMode {
  fn default() -> Self {
    CodeSplittingMode::Bool(true)
  }
}

impl CodeSplittingMode {
  /// Returns true if automatic code splitting is enabled
  pub fn is_automatic(&self) -> bool {
    matches!(self, CodeSplittingMode::Bool(true))
  }

  /// Returns true if dynamic imports should be inlined (no code splitting)
  pub fn is_disabled(&self) -> bool {
    matches!(self, CodeSplittingMode::Bool(false))
  }
}

impl Display for CodeSplittingMode {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      CodeSplittingMode::Bool(true) => write!(f, "enabled"),
      CodeSplittingMode::Bool(false) => write!(f, "disabled"),
    }
  }
}
