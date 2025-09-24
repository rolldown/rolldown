use crate::{NormalizedBundlerOptions, OutputFormat};
use oxc::{
  mangler::{MangleOptions, MangleOptionsKeepNames},
  minifier::{CompressOptions, CompressOptionsKeepNames, TreeShakeOptions},
};
#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::Deserialize;

#[derive(Debug, Clone)]
pub enum RawMinifyOptions {
  Bool(bool),
  DeadCodeEliminationOnly,
  Object((oxc::minifier::MinifierOptions, bool)),
}

impl RawMinifyOptions {
  /// Returns `true` if the minify options is [`Enabled`].
  ///
  /// [`Enabled`]: RawMinifyOptions::Object
  #[must_use]
  pub fn is_enabled(&self) -> bool {
    !matches!(self, Self::Bool(false))
  }
}

impl RawMinifyOptions {
  pub fn normalize(self, options: &NormalizedBundlerOptions) -> MinifyOptions {
    match self {
      RawMinifyOptions::Bool(value) => {
        if value {
          let keep_names = options.keep_names;
          let mangle = MangleOptions {
            // IIFE need to preserve top level names
            top_level: !matches!(options.format, OutputFormat::Iife),
            keep_names: MangleOptionsKeepNames { function: keep_names, class: keep_names },
            debug: false,
          };

          let compress = CompressOptions {
            target: options.transform_options.target.clone(),
            keep_names: CompressOptionsKeepNames { function: keep_names, class: keep_names },
            treeshake: TreeShakeOptions::from(&options.treeshake),
            ..CompressOptions::smallest()
          };
          MinifyOptions::Enabled((
            oxc::minifier::MinifierOptions { mangle: Some(mangle), compress: Some(compress) },
            true,
          ))
        } else {
          MinifyOptions::Disabled
        }
      }
      RawMinifyOptions::DeadCodeEliminationOnly => MinifyOptions::DeadCodeEliminationOnly,
      RawMinifyOptions::Object(value) => MinifyOptions::Enabled(value),
    }
    //
  }
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
  DeadCodeEliminationOnly,
  /// Setting all values to false in `MinifyOptionsObject` means DCE only.
  Enabled((oxc::minifier::MinifierOptions, bool)),
}

impl MinifyOptions {
  /// Returns `true` if the minify options is [`Enabled`].
  ///
  /// [`Enabled`]: MinifyOptions::Enabled
  #[must_use]
  pub fn is_enabled(&self) -> bool {
    !matches!(self, Self::Disabled)
  }
}

/// A simple minify option that can be either a boolean or a string, used for rolldown rust testing.
#[cfg(feature = "deserialize_bundler_options")]
#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(rename_all = "camelCase", deny_unknown_fields, untagged)
)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SimpleMinifyOptions {
  Boolean(bool),
  String(String),
}
