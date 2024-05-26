use oxc::{
  ast::{Comment, CommentKind},
  span::Span,
};

use super::SideEffectDetector;

impl<'a> SideEffectDetector<'a> {
  /// Get the nearest comment before the `span`, return `None` if no leading comment is founded.
  ///
  ///  # Examples
  /// ```javascript
  /// /* valid comment for `a`  */ let a = 1;
  ///
  /// // valid comment for `b`
  /// let b = 1;
  ///
  /// // valid comment for `c`
  ///
  ///
  /// let c = 1;
  ///
  /// let d = 1; /* invalid comment for `e` */
  /// let e = 2
  /// ```
  /// Derived from https://github.com/oxc-project/oxc/blob/147864cfeb112df526bb83d5b8671b465c005066/crates/oxc_linter/src/utils/tree_shaking.rs#L204
  pub fn leading_comment_for(&self, span: Span) -> Option<(&Comment, &str)> {
    let (start, comment) = self.trivias.comments_range(..span.start).next_back()?;

    let comment_text = {
      let span = Span::new(*start, comment.end);
      span.source_text(self.source)
    };

    // If there are non-whitespace characters between the `comment`` and the `span`,
    // we treat the `comment` not belongs to the `span`.
    let only_whitespace = self.source[comment.end as usize..span.start as usize]
      .strip_prefix("*/") // for multi-line comment
      .is_some_and(|s| s.trim().is_empty());

    if !only_whitespace {
      return None;
    }

    // Next step, we need make sure it's not the trailing comment of the previous line.
    let mut current_line_start = span.start as usize;
    for c in self.source[..span.start as usize].chars().rev() {
      if c == '\n' {
        break;
      }

      current_line_start -= c.len_utf8();
    }
    let Ok(current_line_start) = u32::try_from(current_line_start) else {
      return None;
    };

    if comment.end < current_line_start {
      let previous_line = self.source[..comment.end as usize].lines().next_back().unwrap_or("");
      let nothing_before_comment = previous_line
        .trim()
        .strip_prefix(if comment.kind == CommentKind::SingleLine { "//" } else { "/*" })
        .is_some_and(|s| s.trim().is_empty());
      if !nothing_before_comment {
        return None;
      }
    }

    Some((comment, comment_text))
  }
}
