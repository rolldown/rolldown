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
  pub debug: Option<bool>,
}

impl RawMangleOptions {
  #[must_use]
  pub fn into_mangle_options(self) -> MangleOptions {
    let default = MangleOptions::default();
    MangleOptions {
      top_level: self.top_level.or(default.top_level),
      keep_names: self.keep_names.unwrap_or(default.keep_names),
      debug: self.debug.unwrap_or(default.debug),
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
  pub fn into_compress_options(self) -> CompressOptions {
    let default = CompressOptions::default();
    CompressOptions {
      target: self.target.unwrap_or(default.target),
      drop_debugger: self.drop_debugger.unwrap_or(default.drop_debugger),
      drop_console: self.drop_console.unwrap_or(default.drop_console),
      join_vars: self.join_vars.unwrap_or(default.join_vars),
      sequences: self.sequences.unwrap_or(default.sequences),
      unused: self.unused.unwrap_or(default.unused),
      keep_names: self.keep_names.unwrap_or(default.keep_names),
      treeshake: self.treeshake.unwrap_or(default.treeshake),
      drop_labels: self.drop_labels.unwrap_or(default.drop_labels),
      max_iterations: self.max_iterations.or(default.max_iterations),
    }
  }
}

#[derive(Debug, Clone)]
pub enum RawMinifyOptions {
  Bool(bool),
  DeadCodeEliminationOnly,
  Object(RawMinifyOptionsDetailed),
}

#[derive(Debug, Clone)]
pub struct RawMinifyOptionsDetailed {
  pub mangle: Option<RawMangleOptions>,
  pub compress: Option<RawCompressOptions>,
  pub default_target: bool,
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
          let mangle = MangleOptions {
            // IIFE need to preserve top level names
            top_level: Some(!matches!(options.format, OutputFormat::Iife)),
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
        let mangle = value.mangle.map(RawMangleOptions::into_mangle_options);
        let compress = value.compress.map(|c| {
          let mut c = c.into_compress_options();
          if value.default_target {
            c.target = options.transform_options.target.clone();
          }
          c
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
