use std::fmt::Display;

#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::Deserialize;

#[derive(Debug, Clone, Copy)]
#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(rename_all = "kebab-case", deny_unknown_fields)
)]
pub enum Comments {
  /// Don't preserve any comment
  None,
  /// Keep comments as much as possible
  Preserve,
  /// Keep legal comments only
  PreserveLegal,
}

impl Display for Comments {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Comments::None => write!(f, "none"),
      Comments::Preserve => write!(f, "preserve"),
      Comments::PreserveLegal => write!(f, "preserve-legal"),
    }
  }
}
