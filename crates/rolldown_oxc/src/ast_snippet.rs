use oxc::{
  allocator::{self, Allocator},
  ast::ast::{self, Statement},
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

  pub fn binding(&self, name: Atom) -> ast::BindingIdentifier {
    ast::BindingIdentifier { name, ..Dummy::dummy(self.alloc) }
  }

  fn binding_reference(&self, name: Atom) -> ast::IdentifierReference {
    ast::IdentifierReference { name, ..Dummy::dummy(self.alloc) }
  }

  /// `[object].[property]`
  pub fn identifier_member_expression(
    &self,
    object: Atom,
    property: Atom,
  ) -> ast::MemberExpression<'_> {
    ast::MemberExpression::StaticMemberExpression(ast::StaticMemberExpression {
      object: ast::Expression::Identifier(self.binding_reference(object).into_in(self.alloc)),
      property: ast::IdentifierName { name: property, ..Dummy::dummy(self.alloc) },
      ..Dummy::dummy(self.alloc)
    })
  }

  /// `name()`
  pub fn call_expr(&self, name: Atom) -> ast::CallExpression<'_> {
    ast::CallExpression {
      callee: ast::Expression::Identifier(self.binding_reference(name).into_in(self.alloc)),
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

  /// ```js
  ///  var require_foo = __commonJS((exports, module) => {
  ///    ...
  ///  });
  /// ```
  pub fn commonjs_wrapper_stmt(
    &'ast self,
    binding_name: Atom,
    commonjs_name: Atom,
    body: allocator::Vec<'ast, Statement<'ast>>,
  ) -> ast::Statement<'ast> {
    // (exports, module) => {}
    let mut arrow_expr = ast::ArrowExpression {
      body: ast::FunctionBody { statements: body, ..Dummy::dummy(self.alloc) }.into_in(self.alloc),
      ..Dummy::dummy(self.alloc)
    };
    arrow_expr.params.items.push(ast::FormalParameter {
      pattern: ast::BindingPattern {
        kind: ast::BindingPatternKind::BindingIdentifier(
          self.binding("exports".into()).into_in(self.alloc),
        ),
        ..Dummy::dummy(self.alloc)
      },
      ..Dummy::dummy(self.alloc)
    });
    arrow_expr.params.items.push(ast::FormalParameter {
      pattern: ast::BindingPattern {
        kind: ast::BindingPatternKind::BindingIdentifier(
          self.binding("module".into()).into_in(self.alloc),
        ),
        ..Dummy::dummy(self.alloc)
      },
      ..Dummy::dummy(self.alloc)
    });

    //  __commonJS(...)
    let mut commonjs_call_expr = self.call_expr(commonjs_name);
    commonjs_call_expr.arguments.push(ast::Argument::Expression(ast::Expression::ArrowExpression(
      arrow_expr.into_in(self.alloc),
    )));

    // var require_foo = ...

    let var_decl_stmt = self.var_decl_stmt(
      binding_name,
      ast::Expression::CallExpression(commonjs_call_expr.into_in(self.alloc)),
    );

    var_decl_stmt
  }

  /// ```js
  /// var init_foo = __esm(() => { ... });
  /// ```
  pub fn esm_wrapper_stmt(
    &'ast self,
    binding_name: Atom,
    esm_fn_name: Atom,
    body: allocator::Vec<'ast, Statement<'ast>>,
  ) -> ast::Statement<'ast> {
    // () => { ... }
    let arrow_expr = ast::ArrowExpression {
      body: ast::FunctionBody { statements: body, ..Dummy::dummy(self.alloc) }.into_in(self.alloc),
      ..Dummy::dummy(self.alloc)
    };

    //  __esm(...)
    let mut commonjs_call_expr = self.call_expr(esm_fn_name);
    commonjs_call_expr.arguments.push(ast::Argument::Expression(ast::Expression::ArrowExpression(
      arrow_expr.into_in(self.alloc),
    )));

    // var init_foo = ...

    self.var_decl_stmt(
      binding_name,
      ast::Expression::CallExpression(commonjs_call_expr.into_in(self.alloc)),
    )
  }
}
