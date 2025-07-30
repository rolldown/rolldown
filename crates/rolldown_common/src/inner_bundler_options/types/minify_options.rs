use oxc::{
  mangler::{MangleOptions, MangleOptionsKeepNames},
  minifier::{CompressOptions, CompressOptionsKeepNames, TreeShakeOptions},
};
#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::Deserialize;

use crate::{NormalizedBundlerOptions, OutputFormat};

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
            drop_console: true,
            drop_debugger: true,
            join_vars: true,
            sequences: true,
            remove_whitespace: true,
          })
        } else {
          Self::Disabled
        }
      }
      RawMinifyOptions::DeadCodeEliminationOnly => Self::Enabled(MinifyOptionsObject {
        mangle: false,
        compress: false,
        drop_console: false,
        drop_debugger: false,
        join_vars: false,
        sequences: false,
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
  pub drop_console: bool,
  pub drop_debugger: bool,
  pub join_vars: bool,
  pub sequences: bool,
  pub remove_whitespace: bool,
}
impl MinifyOptionsObject {
  pub fn to_oxc_minifier_options(
    &self,
    option: &NormalizedBundlerOptions,
  ) -> oxc::minifier::MinifierOptions {
    let keep_names = option.keep_names;
    oxc::minifier::MinifierOptions {
      mangle: self.mangle.then_some(MangleOptions {
        // IIFE need to preserve top level names
        top_level: !matches!(option.format, OutputFormat::Iife),
        keep_names: MangleOptionsKeepNames { function: keep_names, class: keep_names },
        debug: false,
      }),
      compress: Some(CompressOptions {
        target: option.transform_options.es_target,
        keep_names: CompressOptionsKeepNames { function: keep_names, class: keep_names },
        treeshake: TreeShakeOptions::from(&option.treeshake),
        drop_console: self.drop_console,
        drop_debugger: self.drop_debugger,
        join_vars: self.join_vars,
        sequences: self.sequences,
        ..CompressOptions::smallest()
      })
      .filter(|_| self.compress),
    }
  }
}
