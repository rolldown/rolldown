#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::Deserialize;

#[derive(Debug)]
#[cfg_attr(feature = "deserialize_bundler_options", derive(Deserialize, JsonSchema))]
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
