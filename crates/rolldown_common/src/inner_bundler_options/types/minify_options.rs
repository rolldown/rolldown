use crate::{NormalizedBundlerOptions, OutputFormat};
use oxc::{
  mangler::{MangleOptions, MangleOptionsKeepNames},
  minifier::{CompressOptions, CompressOptionsKeepNames, CompressOptionsUnused, TreeShakeOptions},
  transformer::EngineTargets,
};
use rustc_hash::FxHashSet;
#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::Deserialize;

#[derive(Default, Debug, Clone)]
pub struct RawMangleOptions {
  pub top_level: Option<bool>,
  pub keep_names: Option<MangleOptionsKeepNames>,
}

impl RawMangleOptions {
  #[must_use]
  pub fn into_mangle_options(self, keep_names: bool, output_format: OutputFormat) -> MangleOptions {
    MangleOptions {
      // IIFE need to preserve top level names
      top_level: Some(self.top_level.unwrap_or(!matches!(output_format, OutputFormat::Iife))),
      keep_names: self.keep_names.unwrap_or(if keep_names {
        MangleOptionsKeepNames::all_true()
      } else {
        MangleOptionsKeepNames::all_false()
      }),
      debug: false,
    }
  }
}

#[derive(Default, Debug, Clone)]
pub struct RawCompressOptions {
  pub target: Option<EngineTargets>,
  pub drop_debugger: Option<bool>,
  pub drop_console: Option<bool>,
  pub join_vars: Option<bool>,
  pub sequences: Option<bool>,
  pub unused: Option<CompressOptionsUnused>,
  pub keep_names: Option<CompressOptionsKeepNames>,
  pub treeshake: Option<TreeShakeOptions>,
  pub drop_labels: Option<FxHashSet<String>>,
  pub max_iterations: Option<u8>,
}

impl RawCompressOptions {
  #[must_use]
  pub fn into_compress_options(
    self,
    target: EngineTargets,
    keep_names: bool,
    treeshake: TreeShakeOptions,
  ) -> CompressOptions {
    let smallest = CompressOptions::smallest();
    CompressOptions {
      target: self.target.unwrap_or(target),
      drop_debugger: self.drop_debugger.unwrap_or(smallest.drop_debugger),
      drop_console: self.drop_console.unwrap_or(smallest.drop_console),
      join_vars: self.join_vars.unwrap_or(smallest.join_vars),
      sequences: self.sequences.unwrap_or(smallest.sequences),
      unused: self.unused.unwrap_or(smallest.unused),
      keep_names: self.keep_names.unwrap_or(if keep_names {
        CompressOptionsKeepNames::all_true()
      } else {
        CompressOptionsKeepNames::all_false()
      }),
      treeshake: self.treeshake.unwrap_or(treeshake),
      drop_labels: self.drop_labels.unwrap_or(smallest.drop_labels),
      max_iterations: self.max_iterations.or(smallest.max_iterations),
    }
  }
}

#[derive(Debug, Clone, Default)]
pub enum RawMinifyOptions {
  Bool(bool),
  #[default]
  DeadCodeEliminationOnly,
  Object(RawMinifyOptionsDetailed),
}

#[derive(Debug, Clone)]
pub struct RawMinifyOptionsDetailed {
  pub mangle: Option<RawMangleOptions>,
  pub compress: Option<RawCompressOptions>,
  pub remove_whitespace: bool,
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
          let mangle = RawMangleOptions::default().into_mangle_options(keep_names, options.format);
          let compress = RawCompressOptions::default().into_compress_options(
            options.transform_options.target.clone(),
            keep_names,
            TreeShakeOptions::from(&options.treeshake),
          );
          MinifyOptions::Enabled((
            oxc::minifier::MinifierOptions { mangle: Some(mangle), compress: Some(compress) },
            true,
          ))
        } else {
          MinifyOptions::Disabled
        }
      }
      RawMinifyOptions::DeadCodeEliminationOnly => {
        MinifyOptions::DeadCodeEliminationOnly(oxc::minifier::MinifierOptions {
          mangle: None,
          compress: Some(CompressOptions {
            // For `dce-only`, disable all syntax transforming optimizations
            target: EngineTargets::from_target("es2015").expect("es2015 to be a valid target"),
            treeshake: TreeShakeOptions::from(&options.treeshake),
            ..CompressOptions::dce()
          }),
        })
      }
      RawMinifyOptions::Object(value) => {
        let mangle =
          value.mangle.map(|m| m.into_mangle_options(options.keep_names, options.format));
        let compress = value.compress.map(|c| {
          c.into_compress_options(
            options.transform_options.target.clone(),
            options.keep_names,
            TreeShakeOptions::from(&options.treeshake),
          )
        });
        MinifyOptions::Enabled((
          oxc::minifier::MinifierOptions { mangle, compress },
          value.remove_whitespace,
        ))
      }
    }
    //
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
  DeadCodeEliminationOnly(oxc::minifier::MinifierOptions),
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

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn object_all_true_equals_bool_true() {
    let options = NormalizedBundlerOptions::default();

    let from_bool = RawMinifyOptions::Bool(true).normalize(&options);
    let from_object = RawMinifyOptions::Object(RawMinifyOptionsDetailed {
      mangle: Some(RawMangleOptions::default()),
      compress: Some(RawCompressOptions::default()),
      remove_whitespace: true,
    })
    .normalize(&options);

    assert_eq!(format!("{from_bool:?}"), format!("{from_object:?}"));
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
