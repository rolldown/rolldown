use oxc::{allocator::Allocator, ast::ast, span::Atom};

use crate::{Dummy, IntoIn};

// `AstBuilder` is more suitable name, but it's already used in oxc.
pub struct AstSnippet<'me> {
  pub alloc: &'me Allocator,
}

impl<'me> AstSnippet<'me> {
  pub fn new(alloc: &'me Allocator) -> Self {
    Self { alloc }
  }

  fn identifier_reference(&self, name: Atom) -> ast::IdentifierReference {
    ast::IdentifierReference { name, ..Dummy::dummy(self.alloc) }
  }

  pub fn identifier_member_expression(
    &self,
    object: Atom,
    property: Atom,
  ) -> ast::MemberExpression<'_> {
    ast::MemberExpression::StaticMemberExpression(ast::StaticMemberExpression {
      object: ast::Expression::Identifier(self.identifier_reference(object).into_in(self.alloc)),
      property: ast::IdentifierName { name: property, ..Dummy::dummy(self.alloc) },
      ..Dummy::dummy(self.alloc)
    })
  }
}
