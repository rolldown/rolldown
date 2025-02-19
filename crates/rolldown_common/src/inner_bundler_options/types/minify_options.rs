use oxc::{mangler::MangleOptions, minifier::CompressOptions, transformer::ESTarget};
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

impl MinifyOptions {
  /// Returns `true` if the minify options is [`Enabled`].
  ///
  /// [`Enabled`]: MinifyOptions::Enabled
  #[must_use]
  pub fn is_enabled(&self) -> bool {
    matches!(self, Self::Enabled(..))
  }
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
impl MinifyOptionsObject {
  pub fn to_oxc_minifier_options(&self, target: ESTarget) -> oxc::minifier::MinifierOptions {
    oxc::minifier::MinifierOptions {
      mangle: self.mangle.then_some(MangleOptions { top_level: true, debug: false }),
      compress: Some(CompressOptions { target, drop_debugger: true, drop_console: true })
        .filter(|_| self.compress),
    }
  }
}
