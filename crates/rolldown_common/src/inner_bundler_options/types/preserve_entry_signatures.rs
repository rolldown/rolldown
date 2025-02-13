#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::Deserialize;

#[derive(Debug, Clone, Copy)]
#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(rename_all = "camelCase", deny_unknown_fields)
)]
pub enum PreserveEntrySignature {
  Strict,
  AllowExtension,
  ExportsOnly,
  False,
}

impl TryFrom<bool> for PreserveEntrySignature {
  type Error = String;

  fn try_from(value: bool) -> Result<Self, Self::Error> {
    if value {
      Err(format!("Error preserveEntrySignature: {value:?}"))
    } else {
      Ok(Self::False)
    }
  }
}

impl TryFrom<&str> for PreserveEntrySignature {
  type Error = String;

  fn try_from(value: &str) -> Result<Self, Self::Error> {
    match value {
      "strict" => Ok(Self::Strict),
      "allow-extension" => Ok(Self::AllowExtension),
      "exports-only" => Ok(Self::ExportsOnly),
      _ => Err(format!("unknown preserveEntrySignature: {value:?}")),
    }
  }
}
