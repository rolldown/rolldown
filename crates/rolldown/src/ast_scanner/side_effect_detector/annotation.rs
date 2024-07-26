use daachorse::DoubleArrayAhoCorasick;
use oxc::span::Span;
use std::sync::LazyLock;

use super::SideEffectDetector;

static PURE_COMMENTS: LazyLock<DoubleArrayAhoCorasick<usize>> = LazyLock::new(|| {
  let patterns = vec!["@__PURE__", "#__PURE__"];

  DoubleArrayAhoCorasick::new(patterns).unwrap()
});

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

    leading_comment.map_or(false, |(_comment, comment_text)| {
      PURE_COMMENTS.find_iter(comment_text).next().is_some()
    })
  }
}
