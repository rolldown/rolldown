#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::Deserialize;

#[derive(Debug, Clone, Copy, Default)]
#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(rename_all = "lowercase")
)]
pub enum OutputCharset {
  Ascii,
  #[default]
  Utf8,
}

impl OutputCharset {
  #[inline]
  pub fn is_ascii(self) -> bool {
    matches!(self, Self::Ascii)
  }
}
