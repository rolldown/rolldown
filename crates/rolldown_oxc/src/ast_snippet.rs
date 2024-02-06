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

  pub fn id(&self, name: Atom) -> ast::BindingIdentifier {
    ast::BindingIdentifier { name, ..Dummy::dummy(self.alloc) }
  }

  pub fn id_ref(&self, name: Atom) -> ast::IdentifierReference {
    ast::IdentifierReference { name, ..Dummy::dummy(self.alloc) }
  }

  pub fn id_name(&self, name: Atom) -> ast::IdentifierName {
    ast::IdentifierName { name, ..Dummy::dummy(self.alloc) }
  }

  pub fn id_ref_expr(&self, name: Atom) -> ast::Expression<'_> {
    ast::Expression::Identifier(self.id_ref(name).into_in(self.alloc))
  }

  /// `[object].[property]`
  pub fn literal_prop_access_member_expr(
    &self,
    object: Atom,
    property: Atom,
  ) -> ast::MemberExpression<'_> {
    ast::MemberExpression::StaticMemberExpression(ast::StaticMemberExpression {
      object: ast::Expression::Identifier(self.id_ref(object).into_in(self.alloc)),
      property: ast::IdentifierName { name: property, ..Dummy::dummy(self.alloc) },
      ..Dummy::dummy(self.alloc)
    })
  }

  /// `[object].[property]`
  pub fn literal_prop_access_member_expr_expr(
    &self,
    object: Atom,
    property: Atom,
  ) -> ast::Expression<'_> {
    ast::Expression::MemberExpression(
      self.literal_prop_access_member_expr(object, property).into_in(self.alloc),
    )
  }

  /// `name()`
  pub fn call_expr(&self, name: Atom) -> ast::CallExpression<'_> {
    ast::CallExpression {
      callee: ast::Expression::Identifier(self.id_ref(name).into_in(self.alloc)),
      arguments: allocator::Vec::new_in(self.alloc),
      ..Dummy::dummy(self.alloc)
    }
  }

  /// `name()`
  pub fn call_expr_expr(&self, name: Atom) -> ast::Expression<'_> {
    ast::Expression::CallExpression(self.call_expr(name).into_in(self.alloc))
  }

  /// `name(arg)`
  pub fn call_expr_with_arg_expr(&self, name: Atom, arg: Atom) -> ast::Expression<'_> {
    let arg =
      ast::Argument::Expression(ast::Expression::Identifier(self.id_ref(arg).into_in(self.alloc)));
    let mut call_expr = self.call_expr(name);
    call_expr.arguments.push(arg);
    ast::Expression::CallExpression(call_expr.into_in(self.alloc))
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
          self.id("exports".into()).into_in(self.alloc),
        ),
        ..Dummy::dummy(self.alloc)
      },
      ..Dummy::dummy(self.alloc)
    });
    arrow_expr.params.items.push(ast::FormalParameter {
      pattern: ast::BindingPattern {
        kind: ast::BindingPatternKind::BindingIdentifier(
          self.id("module".into()).into_in(self.alloc),
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

  /// ```js
  /// (a, b)
  /// ```
  pub fn seq2_in_paren_expr(
    &self,
    a: ast::Expression<'ast>,
    b: ast::Expression<'ast>,
  ) -> ast::Expression<'ast> {
    let mut expressions = allocator::Vec::new_in(self.alloc);
    expressions.push(a);
    expressions.push(b);
    let seq_expr = ast::Expression::SequenceExpression(
      ast::SequenceExpression { expressions, ..Dummy::dummy(self.alloc) }.into_in(self.alloc),
    );
    ast::Expression::ParenthesizedExpression(
      ast::ParenthesizedExpression { expression: seq_expr, ..Dummy::dummy(self.alloc) }
        .into_in(self.alloc),
    )
  }

  /// ```js
  /// 42
  /// ```
  pub fn number_expr(&self, value: f64) -> ast::Expression<'ast> {
    ast::Expression::NumberLiteral(
      ast::NumberLiteral {
        span: Dummy::dummy(self.alloc),
        value,
        raw: self.alloc.alloc(value.to_string()),
        base: oxc::syntax::NumberBase::Decimal,
      }
      .into_in(self.alloc),
    )
  }

  /// ```js
  ///  id = ...
  /// ￣￣ AssignmentTarget
  /// ```
  pub fn simple_id_assignment_target(&self, id: Atom) -> ast::AssignmentTarget<'ast> {
    ast::AssignmentTarget::SimpleAssignmentTarget(
      ast::SimpleAssignmentTarget::AssignmentTargetIdentifier(self.id_ref(id).into_in(self.alloc)),
    )
  }

  // `() => xx`
  pub fn only_return_arrow_expr(&self, expr: ast::Expression<'ast>) -> ast::Expression<'ast> {
    let mut statements = allocator::Vec::new_in(self.alloc);
    statements.reserve_exact(1);
    statements.push(ast::Statement::ExpressionStatement(
      ast::ExpressionStatement { expression: expr, ..Dummy::dummy(self.alloc) }.into_in(self.alloc),
    ));
    ast::Expression::ArrowExpression(
      ast::ArrowExpression {
        expression: true,
        body: ast::FunctionBody { statements, ..Dummy::dummy(self.alloc) }.into_in(self.alloc),
        ..Dummy::dummy(self.alloc)
      }
      .into_in(self.alloc),
    )
  }
}
