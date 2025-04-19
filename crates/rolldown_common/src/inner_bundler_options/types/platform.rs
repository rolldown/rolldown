use std::fmt::Display;

#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::Deserialize;

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "deserialize_bundler_options", derive(Deserialize, JsonSchema))]
#[cfg_attr(feature = "deserialize_bundler_options", serde(rename_all = "camelCase"))]
pub enum Platform {
  /// Represents the Node.js platform.
  Node,
  Browser,
  Neutral,
}

impl TryFrom<&str> for Platform {
  type Error = String;

  fn try_from(value: &str) -> Result<Self, Self::Error> {
    match value {
      "node" => Ok(Self::Node),
      "browser" => Ok(Self::Browser),
      "neutral" => Ok(Self::Neutral),
      _ => Err(format!("Unknown platform: {value:?}")),
    }
  }
}

impl Display for Platform {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Node => write!(f, "node"),
      Self::Browser => write!(f, "browser"),
      Self::Neutral => write!(f, "neutral"),
    }
  }
}
