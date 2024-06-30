use std::str::FromStr;

#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::Deserialize;

#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(rename_all = "camelCase", deny_unknown_fields)
)]
#[derive(Debug, Clone, Copy)]
pub enum ModuleType {
  Js,
  Jsx,
  Ts,
  Tsx,
  Json,
  Text,
  Base64,
  DataUrl,
  Binary,
  Empty,
}

impl FromStr for ModuleType {
  type Err = String;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s {
      "js" => Ok(Self::Js),
      "jsx" => Ok(Self::Jsx),
      "ts" => Ok(Self::Ts),
      "tsx" => Ok(Self::Tsx),
      "json" => Ok(Self::Json),
      "text" => Ok(Self::Text),
      "base64" => Ok(Self::Base64),
      "binary" => Ok(Self::Binary),
      "empty" => Ok(Self::Empty),
      _ => Err(format!("Unknown module type: {s}")),
    }
  }
}
