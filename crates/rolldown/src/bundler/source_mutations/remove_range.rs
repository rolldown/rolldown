use oxc::span::Span;

use super::SourceMutation;

#[derive(Debug)]
pub struct RemoveRange {
  pub span: Span,
}

impl SourceMutation for RemoveRange {
  fn apply<'me>(&'me self, _ctx: &super::Context, s: &mut string_wizard::MagicString<'me>) {
    s.remove(self.span.start, self.span.end);
  }
}
