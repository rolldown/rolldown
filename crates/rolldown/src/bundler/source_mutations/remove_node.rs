use oxc::span::Span;

use super::SourceMutation;

pub struct RemoveNode {
  pub span: Span,
}

impl SourceMutation for RemoveNode {
  fn apply<'me>(&'me self, _ctx: &super::Context, s: &mut string_wizard::MagicString<'me>) {
    s.remove(self.span.start, self.span.end);
  }
}
