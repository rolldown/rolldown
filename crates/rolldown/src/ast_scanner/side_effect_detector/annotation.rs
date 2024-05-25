use once_cell::sync::Lazy;
use oxc::span::Span;

use super::SideEffectDetector;

static PURE_COMMENTS: Lazy<regex::Regex> =
  Lazy::new(|| regex::Regex::new("^\\s*(#|@)__PURE__\\s*$").expect("Should create the regex"));

impl<'a> SideEffectDetector<'a> {
  /// Comments containing @__PURE__ or #__PURE__ mark a specific function call
  /// or constructor invocation as side effect free.
  ///
  /// Such an annotation is considered valid if it directly
  /// precedes a function call or constructor invocation
  /// and is only separated from the callee by white-space or comments.
  ///
  /// The only exception are parentheses that wrap a call or invocation.
  ///
  /// <https://rollupjs.org/configuration-options/#pure>
  /// Derived from https://github.com/oxc-project/oxc/blob/147864cfeb112df526bb83d5b8671b465c005066/crates/oxc_linter/src/utils/tree_shaking.rs#L162-L171
  pub fn is_pure_function_or_constructor_call(&self, span: Span) -> bool {
    let leading_comment = self.leading_comment_for(span);

    leading_comment.map_or(false, |(_comment, comment_text)| PURE_COMMENTS.is_match(comment_text))
  }
}
