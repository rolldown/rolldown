#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::Deserialize;

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
  /// // foo.js
  /// export const long_string = 'this is a very long string that will be inlined everywhere';
  /// ```
  pub inline_const: Option<InlineConstOption>,
}

#[derive(Debug, Clone)]
#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(rename_all = "camelCase", deny_unknown_fields, untagged)
)]
pub enum InlineConstOption {
  Bool(bool),
  Option(InlineConstOptionInner),
}

#[derive(Debug, Clone)]
#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(rename_all = "camelCase", deny_unknown_fields)
)]
pub struct InlineConstOptionInner {
  pub pass: u8,
}

impl Default for InlineConstOption {
  fn default() -> Self {
    Self::Bool(false)
  }
}

impl OptimizationOption {
  #[inline]
  pub fn is_inline_const_enabled(&self) -> bool {
    matches!(
      self.inline_const,
      Some(InlineConstOption::Bool(true) | InlineConstOption::Option { .. })
    )
  }

  pub fn inline_const_pass(&self) -> u8 {
    match self.inline_const {
      Some(InlineConstOption::Bool(true)) => 1,
      Some(InlineConstOption::Option(ref inner)) => inner.pass,
      _ => 0,
    }
  }
}
