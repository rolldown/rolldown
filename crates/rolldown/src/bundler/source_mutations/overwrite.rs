use oxc::span::Span;
use string_wizard::UpdateOptions;

use super::SourceMutation;

#[derive(Debug)]
pub struct Overwrite {
  pub span: Span,
  pub content: String,
}

impl SourceMutation for Overwrite {
  fn apply<'me>(&'me self, _ctx: &super::Context, s: &mut string_wizard::MagicString<'me>) {
    s.update_with(
      self.span.start,
      self.span.end,
      &self.content,
      UpdateOptions { overwrite: true, ..Default::default() },
    );
  }
}
