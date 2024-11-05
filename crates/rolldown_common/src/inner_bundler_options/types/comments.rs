#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::Deserialize;

#[derive(Debug)]
#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(rename_all = "camelCase", deny_unknown_fields)
)]
pub enum Comments {
  /// Don't preserve any comment
  None,
  /// Keep comments as much as possible
  Preserve,
  /// Keep legal comments only
  PreserveLegalComments,
}
