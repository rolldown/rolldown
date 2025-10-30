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
  /// Preserve legal comments (marked with @license, @preserve, //!, /*!)
  Inline,
  /// Preserve all comments including JSDoc and legal comments
  All,
}

impl Display for Comments {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Comments::None => write!(f, "none"),
      Comments::Inline => write!(f, "inline"),
      Comments::All => write!(f, "all"),
    }
  }
}
