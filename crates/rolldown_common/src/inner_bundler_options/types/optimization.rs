#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::{Deserialize, Deserializer};

use crate::Platform;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(rename_all = "camelCase", deny_unknown_fields)
)]
pub struct InlineConstConfig {
  #[cfg_attr(feature = "deserialize_bundler_options", serde(default))]
  pub mode: Option<InlineConstMode>,
  #[cfg_attr(feature = "deserialize_bundler_options", serde(default = "default_pass"))]
  pub pass: u32,
}

impl Default for InlineConstConfig {
  fn default() -> Self {
    Self { mode: Some(InlineConstMode::All), pass: 1 }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(rename_all = "camelCase")
)]
pub enum InlineConstMode {
  #[default]
  All,
  Smart,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(rename_all = "camelCase", deny_unknown_fields),
  serde(untagged)
)]
pub enum InlineConstOption {
  Bool(bool),
  Config(InlineConstConfig),
}

#[cfg(feature = "deserialize_bundler_options")]
fn default_pass() -> u32 {
  1
}

impl Default for InlineConstOption {
  fn default() -> Self {
    InlineConstOption::Bool(false)
  }
}

#[derive(Debug, Default, Clone)]
#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(rename_all = "camelCase", deny_unknown_fields)
)]
pub struct OptimizationOption {
  /// Inline constant everywhere not always generate smaller bundle, e.g.
  /// ```js
  /// // index.js
  /// import {long_string} from './foo.js'
  /// console.log(long_string);
  /// console.log(long_string);
  /// console.log(long_string);
  /// console.log(long_string);
  /// console.log(long_string);
  /// // foo.js
  /// export const long_string = 'this is a very long string that will be inlined everywhere';
  /// ```
  ///
  /// Options:
  /// - `None`: Use default behavior (false)
  /// - `Some(InlineConstOption::Bool(false))`: Disable inlining
  /// - `Some(InlineConstOption::Bool(true))`: Inline everywhere
  /// - `Some(InlineConstOption::Config({ mode: Some(Smart), .. }))`: Only inline when the output is speculated to be smaller
  /// - `Some(InlineConstOption::Config({ mode: Some(All), pass: n }))`: Inline everywhere with n passes
  ///
  #[cfg_attr(
    feature = "deserialize_bundler_options",
    serde(deserialize_with = "deserialize_inline_const", default)
  )]
  pub inline_const: Option<InlineConstOption>,
  /// Use PIFE patterns for module wrappers so that JS engines can compile the functions eagerly.
  /// This improves the initial execution performance.
  /// See <https://v8.dev/blog/preparser#pife> for more details about the optimization.
  pub pife_for_module_wrappers: Option<bool>,
}

pub fn normalize_optimization_option(
  option: Option<OptimizationOption>,
  platform: Platform,
) -> NormalizedOptimizationConfig {
  let option = option.unwrap_or_default();
  let inline_const = option.inline_const.and_then(|inline_const| match inline_const {
    InlineConstOption::Bool(true) => {
      Some(NormalizedInlineConstConfig { mode: InlineConstMode::All, pass: 1 })
    }
    InlineConstOption::Bool(false) => None,
    InlineConstOption::Config(config) => {
      let mode = config.mode.unwrap_or(InlineConstMode::All);
      let pass = config.pass;
      Some(NormalizedInlineConstConfig { mode, pass })
    }
  });

  NormalizedOptimizationConfig {
    inline_const,
    pife_for_module_wrappers: option
      .pife_for_module_wrappers
      .unwrap_or(!matches!(platform, Platform::Neutral)),
  }
}

#[cfg(feature = "deserialize_bundler_options")]
pub fn deserialize_inline_const<'de, D>(
  deserializer: D,
) -> Result<Option<InlineConstOption>, D::Error>
where
  D: Deserializer<'de>,
{
  use serde_json::Value;

  let deserialized = Option::<Value>::deserialize(deserializer)?;
  match deserialized {
    Some(Value::Bool(v)) => Ok(Some(InlineConstOption::Bool(v))),
    Some(Value::Object(obj)) => {
      let config =
        InlineConstConfig::deserialize(Value::Object(obj)).map_err(serde::de::Error::custom)?;
      Ok(Some(InlineConstOption::Config(config)))
    }
    None => Ok(None),
    _ => unreachable!(),
  }
}

#[derive(Debug, Clone, Default)]
pub struct NormalizedOptimizationConfig {
  pub inline_const: Option<NormalizedInlineConstConfig>,
  pub pife_for_module_wrappers: bool,
}

#[derive(Debug, Clone, Default, Copy)]
pub struct NormalizedInlineConstConfig {
  pub mode: InlineConstMode,
  pub pass: u32,
}

impl NormalizedOptimizationConfig {
  #[inline]
  pub fn is_inline_const_enabled(&self) -> bool {
    self.inline_const.is_some()
  }

  #[inline]
  pub fn is_inline_const_smart_mode(&self) -> bool {
    matches!(self.inline_const, Some(ref inline_const) if inline_const.mode == InlineConstMode::Smart)
  }

  #[inline]
  pub fn inline_const_pass(&self) -> u32 {
    self.inline_const.map(|item| item.pass).unwrap_or(1)
  }

  #[inline]
  pub fn is_pife_for_module_wrappers_enabled(&self) -> bool {
    self.pife_for_module_wrappers
  }
}
