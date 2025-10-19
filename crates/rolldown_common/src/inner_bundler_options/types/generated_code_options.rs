#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::Deserialize;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "deserialize_bundler_options", derive(Deserialize, JsonSchema))]
#[cfg_attr(feature = "deserialize_bundler_options", serde(rename_all = "camelCase"))]
pub struct GeneratedCodeOptions {
  pub preset: Option<GeneratedCodePreset>,
  // pub arrow_functions: bool,
  // pub const_bindings: bool,
  // pub object_shorthand: bool,
  // pub reserved_names_as_props: bool,
  pub symbols: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "deserialize_bundler_options", derive(Deserialize, JsonSchema))]
#[cfg_attr(feature = "deserialize_bundler_options", serde(rename_all = "camelCase"))]
pub enum GeneratedCodePreset {
  Es5,
  Es2015,
}

impl GeneratedCodeOptions {
  pub fn es5() -> Self {
    Self { symbols: false, preset: None }
  }

  pub fn es2015() -> Self {
    Self { symbols: true, preset: None }
  }
}
