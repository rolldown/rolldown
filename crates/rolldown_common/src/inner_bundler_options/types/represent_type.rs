use std::fmt::Display;

/// Describes how a module is represented without activating representation
/// behavior. See internal-docs/module-representation/implementation.md.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepresentType {
  Js,
  Json,
  Text,
  Base64,
  Dataurl,
  Binary,
  Empty,
  Url,
  Copy,
}

impl TryFrom<&str> for RepresentType {
  type Error = anyhow::Error;

  fn try_from(value: &str) -> Result<Self, Self::Error> {
    match value {
      "js" => Ok(Self::Js),
      "json" => Ok(Self::Json),
      "text" => Ok(Self::Text),
      "base64" => Ok(Self::Base64),
      "dataurl" => Ok(Self::Dataurl),
      "binary" => Ok(Self::Binary),
      "empty" => Ok(Self::Empty),
      "url" => Ok(Self::Url),
      "copy" => Ok(Self::Copy),
      _ => Err(anyhow::format_err!("Unknown represent type: {value}")),
    }
  }
}

impl Display for RepresentType {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str(match self {
      Self::Js => "js",
      Self::Json => "json",
      Self::Text => "text",
      Self::Base64 => "base64",
      Self::Dataurl => "dataurl",
      Self::Binary => "binary",
      Self::Empty => "empty",
      Self::Url => "url",
      Self::Copy => "copy",
    })
  }
}

#[cfg(test)]
mod tests {
  use super::RepresentType;

  #[test]
  fn parses_all_represent_types() {
    for (value, expected) in [
      ("js", RepresentType::Js),
      ("json", RepresentType::Json),
      ("text", RepresentType::Text),
      ("base64", RepresentType::Base64),
      ("dataurl", RepresentType::Dataurl),
      ("binary", RepresentType::Binary),
      ("empty", RepresentType::Empty),
      ("url", RepresentType::Url),
      ("copy", RepresentType::Copy),
    ] {
      assert_eq!(RepresentType::try_from(value).unwrap(), expected);
      assert_eq!(expected.to_string(), value);
    }
  }

  #[test]
  fn rejects_unknown_represent_type() {
    RepresentType::try_from("asset").unwrap_err();
  }
}
