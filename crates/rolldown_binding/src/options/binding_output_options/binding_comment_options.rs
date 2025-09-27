use napi_derive::napi;

#[napi(object, object_to_js = false)]
#[derive(Debug, Default)]
pub struct BindingCommentOptions {
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
  /// Default is `true`.
  pub legal: Option<bool>,
}

impl From<BindingCommentOptions> for rolldown_common::CommentOptions {
  fn from(opts: BindingCommentOptions) -> Self {
    rolldown_common::CommentOptions {
      normal: opts.normal,
      jsdoc: opts.jsdoc,
      annotation: opts.annotation,
      legal: opts.legal,
    }
  }
}
