#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::Deserialize;

/// What Rolldown does with `import defer` and `import.defer()` of an external module in ESM
/// output (`experimental.importDeferExternal`).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "deserialize_bundler_options", derive(Deserialize, JsonSchema))]
#[cfg_attr(feature = "deserialize_bundler_options", serde(rename_all = "camelCase"))]
pub enum ImportDeferExternal {
  /// A build error. The target runtime can be without support for `import defer`.
  #[default]
  Error,
  /// Keep `import defer` in the output. The target runtime supports it.
  Preserve,
}

impl TryFrom<&str> for ImportDeferExternal {
  type Error = String;

  fn try_from(value: &str) -> Result<Self, String> {
    match value {
      "error" => Ok(ImportDeferExternal::Error),
      "preserve" => Ok(ImportDeferExternal::Preserve),
      _ => Err(format!("Unknown value of `experimental.importDeferExternal`: {value}")),
    }
  }
}
