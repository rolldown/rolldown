use std::fmt;

#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "deserialize_bundler_options", derive(Deserialize, Serialize, JsonSchema))]
pub enum EmptyOutDirMode {
  /// Don't do anything about the out dir.
  Disabled,

  /// Only clean the out dir inside project root.
  Normal,

  /// Clean the out dir even if it's not inside project root.
  Force,
}

impl Default for EmptyOutDirMode {
  fn default() -> Self {
    Self::Disabled
  }
}

impl fmt::Display for EmptyOutDirMode {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Disabled => write!(f, "disabled"),
      Self::Normal => write!(f, "normal"),
      Self::Force => write!(f, "force"),
    }
  }
}

impl From<bool> for EmptyOutDirMode {
  fn from(value: bool) -> Self {
    if value { Self::Normal } else { Self::Disabled }
  }
}

impl From<EmptyOutDirMode> for bool {
  fn from(value: EmptyOutDirMode) -> Self {
    matches!(value, EmptyOutDirMode::Normal | EmptyOutDirMode::Force)
  }
}
