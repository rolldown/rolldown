use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub enum SourceMapType {
  File,
  Inline,
  Hidden,
}

impl SourceMapType {
  pub fn is_hidden(&self) -> bool {
    matches!(self, Self::Hidden)
  }
}

impl From<String> for SourceMapType {
  fn from(value: String) -> Self {
    match value.as_str() {
      "file" => SourceMapType::File,
      "inline" => SourceMapType::Inline,
      "hidden" => SourceMapType::Hidden,
      _ => unreachable!("unknown sourcemap type"),
    }
  }
}
