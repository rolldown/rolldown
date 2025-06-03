#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::Deserialize;
use std::fmt::Display;

#[derive(Debug, Clone, Copy)]
#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(rename_all = "camelCase", deny_unknown_fields)
)]
pub enum OutputFormat {
  Esm,
  Cjs,
  Iife,
  Umd,
}

impl OutputFormat {
  #[inline]
  pub fn keep_esm_import_export_syntax(&self) -> bool {
    matches!(self, Self::Esm)
  }

  #[inline]
  /// https://github.com/evanw/esbuild/blob/d34e79e2a998c21bb71d57b92b0017ca11756912/internal/config/config.go#L664-L666
  /// Since we have different implementation for `IIFE` and extra implementation of `UMD` omit them as well
  pub fn should_call_runtime_require(&self) -> bool {
    !matches!(self, Self::Cjs | Self::Umd | Self::Iife)
  }
}

impl Display for OutputFormat {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Esm => write!(f, "esm"),
      Self::Cjs => write!(f, "cjs"),
      Self::Iife => write!(f, "iife"),
      Self::Umd => write!(f, "umd"),
    }
  }
}
