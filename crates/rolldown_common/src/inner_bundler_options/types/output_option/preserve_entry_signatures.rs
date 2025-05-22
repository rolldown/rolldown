#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::Deserialize;

#[derive(Debug, Default, Clone, Copy)]
#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(rename_all = "kebab-case", deny_unknown_fields)
)]
pub enum PreserveEntrySignatures {
  AllowExtension,
  Strict,
  ExportsOnly,
  #[default]
  False,
}
