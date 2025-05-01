use std::fmt::Display;

#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::Deserialize;

#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(rename_all = "camelCase", deny_unknown_fields)
)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModuleType {
  Js,
  Jsx,
  Ts,
  Tsx,
  Dts,
  Json,
  Text,
  Base64,
  Dataurl,
  Binary,
  Empty,
  Css,
  Asset,
  Custom(String),
}

impl ModuleType {
  pub fn from_known_str(s: &str) -> anyhow::Result<Self> {
    match s {
      "js" => Ok(Self::Js),
      "jsx" => Ok(Self::Jsx),
      "ts" => Ok(Self::Ts),
      "tsx" => Ok(Self::Tsx),
      "json" => Ok(Self::Json),
      "text" => Ok(Self::Text),
      "base64" => Ok(Self::Base64),
      "dataurl" => Ok(Self::Dataurl),
      "binary" => Ok(Self::Binary),
      "empty" => Ok(Self::Empty),
      "css" => Ok(Self::Css),
      "asset" => Ok(Self::Asset),
      _ => Err(anyhow::format_err!("Unknown module type: {s}")),
    }
  }

  /// error: method `from_str` can be confused for the standard trait method `std::str::FromStr::from_str`
  /// to avoid conflicting with std
  pub fn from_str_with_fallback<S: AsRef<str>>(s: S) -> Self {
    match s.as_ref() {
      "js" => Self::Js,
      "jsx" => Self::Jsx,
      "ts" => Self::Ts,
      "tsx" => Self::Tsx,
      "json" => Self::Json,
      "text" => Self::Text,
      "base64" => Self::Base64,
      "dataurl" => Self::Dataurl,
      "binary" => Self::Binary,
      "empty" => Self::Empty,
      "css" => Self::Css,
      "asset" => Self::Asset,
      _ => Self::Custom(s.as_ref().to_string()),
    }
  }
}

impl Display for ModuleType {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      ModuleType::Js => write!(f, "js"),
      ModuleType::Jsx => write!(f, "jsx"),
      ModuleType::Ts => write!(f, "ts"),
      ModuleType::Tsx => write!(f, "tsx"),
      ModuleType::Dts => write!(f, "dts"),
      ModuleType::Json => write!(f, "json"),
      ModuleType::Text => write!(f, "text"),
      ModuleType::Base64 => write!(f, "base64"),
      ModuleType::Dataurl => write!(f, "dataurl"),
      ModuleType::Binary => write!(f, "binary"),
      ModuleType::Empty => write!(f, "empty"),
      ModuleType::Css => write!(f, "css"),
      ModuleType::Asset => write!(f, "asset"),
      ModuleType::Custom(custom_type) => write!(f, "{custom_type}"),
    }
  }
}
