use oxc::{ast::Comment, span::Span};

pub static ROLLDOWN_IGNORE: &str = "@rolldown-ignore";

/// Get the leading comment of a node when condition is satisfy
pub fn get_leading_comment<'a, 'ast: 'a, F: Fn(&Comment) -> bool>(
  comments: &'a oxc::allocator::Vec<'ast, Comment>,
  node_span: Span,
  predicate: Option<F>,
) -> Option<&'a Comment> {
  let i = comments.binary_search_by(|c| c.attached_to.cmp(&node_span.start)).ok()?;
  let comment = comments.get(i)?;
  match predicate {
    Some(predicate) if predicate(comment) => Some(comment),
    _ => None,
  }
}
