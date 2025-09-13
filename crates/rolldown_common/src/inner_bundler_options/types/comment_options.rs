#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::Deserialize;

#[derive(Debug, Clone)]
#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(rename_all = "camelCase", deny_unknown_fields)
)]
pub struct CommentOptions {
  /// Print normal comments that do not have special meanings.
  ///
  /// At present only statement level comments are printed.
  ///
  /// Default is `true`.
  pub normal: Option<bool>,

  /// Print jsdoc comments.
  ///
  /// * jsdoc: `/** jsdoc */`
  ///
  /// Default is `true`.
  pub jsdoc: Option<bool>,

  /// Print annotation comments.
  ///
  /// * pure: `/* #__PURE__ */` and `/* #__NO_SIDE_EFFECTS__ */`
  /// * webpack: `/* webpackChunkName */`
  /// * vite: `/* @vite-ignore */`
  /// * coverage: `v8 ignore`, `c8 ignore`, `node:coverage`, `istanbul ignore`
  ///
  /// Default is `true`.
  pub annotation: Option<bool>,

  /// Print legal comments.
  ///
  /// * starts with `//!` or `/*!`.
  /// * contains `/* @license */` or `/* @preserve */`
  ///
  /// Default is `true` (maps to LegalComments::Inline).
  pub legal: Option<bool>,
}

impl Default for CommentOptions {
  fn default() -> Self {
    Self { normal: Some(false), jsdoc: Some(true), annotation: Some(true), legal: Some(false) }
  }
}

impl CommentOptions {
  #[inline]
  pub fn normal(&self) -> bool {
    self.normal.unwrap_or(false)
  }

  #[inline]
  pub fn legal(&self) -> bool {
    self.legal.unwrap_or(false)
  }

  #[inline]
  pub fn jsdoc(&self) -> bool {
    self.jsdoc.unwrap_or(true)
  }

  #[inline]
  pub fn annotation(&self) -> bool {
    self.annotation.unwrap_or(true)
  }
}
