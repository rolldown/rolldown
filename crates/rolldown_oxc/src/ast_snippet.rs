use oxc::{
  allocator::{self, Allocator},
  ast::ast,
  span::Atom,
};

use crate::{Dummy, IntoIn};

// `AstBuilder` is more suitable name, but it's already used in oxc.
pub struct AstSnippet<'ast> {
  pub alloc: &'ast Allocator,
}

impl<'ast> AstSnippet<'ast> {
  pub fn new(alloc: &'ast Allocator) -> Self {
    Self { alloc }
  }

  fn identifier_reference(&self, name: Atom) -> ast::IdentifierReference {
    ast::IdentifierReference { name, ..Dummy::dummy(self.alloc) }
  }

  /// `[object].[property]`
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

  /// `name()`
  pub fn call_expr(&self, name: Atom) -> ast::CallExpression<'_> {
    ast::CallExpression {
      callee: ast::Expression::Identifier(self.identifier_reference(name).into_in(self.alloc)),
      arguments: allocator::Vec::new_in(self.alloc),
      ..Dummy::dummy(self.alloc)
    }
  }

  /// `name()`
  pub fn call_expr_expr(&self, name: Atom) -> ast::Expression<'_> {
    ast::Expression::CallExpression(self.call_expr(name).into_in(self.alloc))
  }

  /// `name()`
  pub fn call_expr_stmt(&self, name: Atom) -> ast::Statement<'_> {
    ast::Statement::ExpressionStatement(
      ast::ExpressionStatement {
        expression: self.call_expr_expr(name),
        ..Dummy::dummy(self.alloc)
      }
      .into_in(self.alloc),
    )
  }

  /// `var [name] = [init]`
  pub fn var_decl_stmt(&self, name: Atom, init: ast::Expression<'ast>) -> ast::Statement<'ast> {
    ast::Statement::Declaration(self.var_decl(name, init))
  }

  /// `var [name] = [init]`
  pub fn var_decl(&self, name: Atom, init: ast::Expression<'ast>) -> ast::Declaration<'ast> {
    let mut declarations = allocator::Vec::new_in(self.alloc);
    declarations.push(ast::VariableDeclarator {
      id: ast::BindingPattern {
        kind: ast::BindingPatternKind::BindingIdentifier(
          ast::BindingIdentifier { name, ..Dummy::dummy(self.alloc) }.into_in(self.alloc),
        ),
        ..Dummy::dummy(self.alloc)
      },
      init: Some(init),
      ..Dummy::dummy(self.alloc)
    });
    ast::Declaration::VariableDeclaration(
      ast::VariableDeclaration {
        kind: ast::VariableDeclarationKind::Var,
        declarations,
        ..Dummy::dummy(self.alloc)
      }
      .into_in(self.alloc),
    )
  }
}
