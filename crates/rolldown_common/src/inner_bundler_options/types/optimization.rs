#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::{Deserialize, Deserializer};

use crate::Platform;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(rename_all = "camelCase", deny_unknown_fields),
  serde(untagged)
)]
pub enum InlineConstOption {
  Bool(bool),
  #[cfg_attr(feature = "deserialize_bundler_options", schemars(with = "String"))]
  Safe,
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
  /// - `Some(InlineConstOption::Safe("safe"))`: Only inline when safe
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

impl OptimizationOption {
  #[inline]
  pub fn is_inline_const_enabled(&self) -> bool {
    matches!(&self.inline_const, Some(InlineConstOption::Bool(true) | InlineConstOption::Safe))
  }

  #[inline]
  pub fn is_inline_const_safe_mode(&self) -> bool {
    matches!(&self.inline_const, Some(InlineConstOption::Safe))
  }

  #[inline]
  pub fn is_pife_for_module_wrappers_enabled(&self) -> bool {
    self.pife_for_module_wrappers.unwrap_or(false)
  }
}

pub fn normalize_optimization_option(
  option: Option<OptimizationOption>,
  platform: Platform,
) -> OptimizationOption {
  let option = option.unwrap_or_default();
  OptimizationOption {
    inline_const: option.inline_const,
    pife_for_module_wrappers: Some(
      option.pife_for_module_wrappers.unwrap_or(!matches!(platform, Platform::Neutral)),
    ),
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
    Some(Value::String(s)) if s == "safe" => Ok(Some(InlineConstOption::Safe)),
    None => Ok(None),
    _ => unreachable!(),
  }
}
