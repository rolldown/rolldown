use oxc::span::{Atom, Span};

use super::SourceMutation;

#[derive(Debug)]
pub struct RenameSymbol {
  pub span: Span,
  pub name: Atom,
}

impl SourceMutation for RenameSymbol {
  fn apply<'me>(&'me self, _ctx: &super::Context, s: &mut string_wizard::MagicString<'me>) {
    s.update(self.span.start, self.span.end, self.name.as_str());
  }
}
