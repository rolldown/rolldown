#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::Deserialize;

use crate::Platform;

#[derive(Debug, Default, Clone)]
#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(rename_all = "camelCase", deny_unknown_fields)
)]
pub struct OptimizationOption {
  /// TODO: make the inline_const option more fine grained, e.g. `inline_const: false | true |
  /// "on-demand"`.
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
  pub inline_const: Option<bool>,
  /// Use PIFE patterns for module wrappers so that JS engines can compile the functions eagerly.
  /// This improves the initial execution performance.
  /// See <https://v8.dev/blog/preparser#pife> for more details about the optimization.
  pub pife_for_module_wrappers: Option<bool>,
}

impl OptimizationOption {
  #[inline]
  pub fn is_inline_const_enabled(&self) -> bool {
    self.inline_const.unwrap_or(false)
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
