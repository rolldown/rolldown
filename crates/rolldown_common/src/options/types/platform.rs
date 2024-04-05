#[derive(Debug, Clone, Copy)]
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
