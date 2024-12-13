use oxc::mangler::MangleOptions;
#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::Deserialize;

#[derive(Debug, Clone)]
#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(untagged)
)]
pub enum RawMinifyOptions {
  Bool(bool),
  DeadCodeEliminationOnly,
  Object(MinifyOptionsObject),
}

impl Default for RawMinifyOptions {
  fn default() -> Self {
    RawMinifyOptions::Bool(false)
  }
}

impl From<bool> for RawMinifyOptions {
  fn from(value: bool) -> Self {
    RawMinifyOptions::Bool(value)
  }
}

#[derive(Debug, Clone)]
pub enum MinifyOptions {
  Disabled,
  Enabled(MinifyOptionsObject),
}

impl From<RawMinifyOptions> for MinifyOptions {
  fn from(value: RawMinifyOptions) -> Self {
    match value {
      RawMinifyOptions::Bool(value) => {
        if value {
          Self::Enabled(MinifyOptionsObject {
            mangle: true,
            compress: true,
            remove_whitespace: true,
          })
        } else {
          Self::Disabled
        }
      }
      RawMinifyOptions::DeadCodeEliminationOnly => Self::Enabled(MinifyOptionsObject {
        mangle: false,
        compress: false,
        remove_whitespace: false,
      }),
      RawMinifyOptions::Object(value) => Self::Enabled(value),
    }
  }
}

#[derive(Debug, Clone)]
#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(rename_all = "camelCase", deny_unknown_fields)
)]
#[allow(clippy::struct_excessive_bools)]
pub struct MinifyOptionsObject {
  pub mangle: bool,
  pub compress: bool,
  pub remove_whitespace: bool,
}

impl From<&MinifyOptionsObject> for oxc::minifier::MinifierOptions {
  fn from(value: &MinifyOptionsObject) -> Self {
    Self {
      mangle: value.mangle.then(MangleOptions::default),
      compress: oxc::minifier::CompressOptions::default(),
    }
  }
}
