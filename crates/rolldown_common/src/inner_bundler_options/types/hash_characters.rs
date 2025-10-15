use std::fmt::Display;

#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::Deserialize;

#[derive(Debug, Clone, Copy)]
#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(rename_all = "camelCase", deny_unknown_fields)
)]
pub enum HashCharacters {
  Base64,
  Base36,
  Hex,
}

impl HashCharacters {
  pub const fn base(&self) -> u8 {
    match self {
      HashCharacters::Base64 => 64,
      HashCharacters::Base36 => 36,
      HashCharacters::Hex => 16,
    }
  }
}

impl Default for HashCharacters {
  fn default() -> Self {
    Self::Base64
  }
}

impl Display for HashCharacters {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      HashCharacters::Base64 => write!(f, "base64"),
      HashCharacters::Base36 => write!(f, "base36"),
      HashCharacters::Hex => write!(f, "hex"),
    }
  }
}
