use oxc::{ast::Comment, span::Span};

/// Get the leading comment of a node when condition is satisfy
pub fn get_leading_comment<'a, 'ast: 'a, F: Fn(&Comment) -> bool>(
  comments: &'a oxc::allocator::Vec<'ast, Comment>,
  node_span: Span,
  predicate: Option<F>,
) -> Option<&'a Comment> {
  let i = comments.binary_search_by(|c| c.attached_to.cmp(&node_span.start)).ok()?;
  let comment = comments.get(i)?;
  predicate.and_then(|func| func(comment).then_some(comment))
}
