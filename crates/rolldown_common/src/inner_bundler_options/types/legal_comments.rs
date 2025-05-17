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
pub enum LegalComments {
  /// Don't preserve any comment
  None,
  /// Preserve all legal comments
  Inline,
}

impl Display for LegalComments {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      LegalComments::None => write!(f, "none"),
      LegalComments::Inline => write!(f, "inline"),
    }
  }
}
