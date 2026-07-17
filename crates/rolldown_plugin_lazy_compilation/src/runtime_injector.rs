use oxc::{
  allocator::Allocator,
  ast::{
    ast::{
      Argument, BindingIdentifier, BindingPattern, Declaration, Expression, FormalParameter,
      FormalParameterKind, FormalParameters, Function, FunctionBody, IdentifierName,
      MemberExpression, Statement, VariableDeclarator,
    },
    builder::{AstBuilder, NONE},
  },
  ast_visit::{VisitMut, walk_mut},
  span::SPAN,
};

const HELPER_NAME: &str = "__unwrap_lazy_compilation_entry";

pub struct LazyCompilationRuntimeInjector<'ast> {
  ast_builder: AstBuilder<'ast>,
  pub transformed_count: usize,
}

impl<'ast> LazyCompilationRuntimeInjector<'ast> {
  pub fn new(allocator: &'ast Allocator) -> Self {
    Self { ast_builder: AstBuilder::new(allocator), transformed_count: 0 }
  }
}

impl<'ast> VisitMut<'ast> for LazyCompilationRuntimeInjector<'ast> {
  fn visit_expression(&mut self, expr: &mut Expression<'ast>) {
    // First visit children
    walk_mut::walk_expression(self, expr);

    // Then transform import expressions
    if matches!(expr, Expression::ImportExpression(_)) {
      // Transform: import(x) -> import(x).then(__unwrap_lazy_compilation_entry)
      let import_expr =
        std::mem::replace(expr, Expression::new_null_literal(SPAN, &self.ast_builder));

      // Build: import_expr.then(__unwrap_lazy_compilation_entry)
      *expr = Expression::new_call_expression(
        SPAN,
        Expression::from(MemberExpression::new_static_member_expression(
          SPAN,
          import_expr,
          IdentifierName::new(SPAN, "then", &self.ast_builder),
          false,
          &self.ast_builder,
        )),
        NONE,
        oxc::allocator::Vec::from_value_in(
          Argument::from(Expression::new_identifier(SPAN, HELPER_NAME, &self.ast_builder)),
          &self.ast_builder,
        ),
        false,
        &self.ast_builder,
      );

      self.transformed_count += 1;
    }
  }
}

/// Creates the helper function:
/// ```js
/// function __unwrap_lazy_compilation_entry(m) {
///   var e = m['rolldown:exports'];
///   return e ? e : m;
/// }
/// ```
pub fn create_unwrap_lazy_compilation_entry_helper(allocator: &Allocator) -> Statement<'_> {
  let ast_builder = AstBuilder::new(allocator);

  // Parameter: m
  let params = FormalParameters::new(
    SPAN,
    FormalParameterKind::FormalParameter,
    oxc::allocator::Vec::from_value_in(
      FormalParameter::new(
        SPAN,
        oxc::allocator::Vec::new_in(&ast_builder),
        BindingPattern::new_binding_identifier(SPAN, "m", &ast_builder),
        NONE,
        NONE,
        false,
        None,
        false,
        false,
        &ast_builder,
      ),
      &ast_builder,
    ),
    NONE,
    &ast_builder,
  );

  // var e = m['rolldown:exports'];
  let var_decl_stmt = Statement::from(Declaration::new_variable_declaration(
    SPAN,
    oxc::ast::ast::VariableDeclarationKind::Var,
    oxc::allocator::Vec::from_value_in(
      VariableDeclarator::new(
        SPAN,
        oxc::ast::ast::VariableDeclarationKind::Var,
        BindingPattern::new_binding_identifier(SPAN, "e", &ast_builder),
        NONE,
        Some(Expression::from(MemberExpression::new_computed_member_expression(
          SPAN,
          Expression::new_identifier(SPAN, "m", &ast_builder),
          Expression::new_string_literal(SPAN, "rolldown:exports", None, &ast_builder),
          false,
          &ast_builder,
        ))),
        false,
        &ast_builder,
      ),
      &ast_builder,
    ),
    false,
    &ast_builder,
  ));

  // return e ? e : m;
  let return_stmt = Statement::new_return_statement(
    SPAN,
    Some(Expression::new_conditional_expression(
      SPAN,
      Expression::new_identifier(SPAN, "e", &ast_builder),
      Expression::new_identifier(SPAN, "e", &ast_builder),
      Expression::new_identifier(SPAN, "m", &ast_builder),
      &ast_builder,
    )),
    &ast_builder,
  );

  // Function body with both statements
  let mut body_stmts = oxc::allocator::Vec::with_capacity_in(2, &ast_builder);
  body_stmts.push(var_decl_stmt);
  body_stmts.push(return_stmt);
  let body =
    FunctionBody::new(SPAN, oxc::allocator::Vec::new_in(&ast_builder), body_stmts, &ast_builder);

  // function __unwrap_lazy_compilation_entry(m) { ... }
  Statement::FunctionDeclaration(Function::boxed(
    SPAN,
    oxc::ast::ast::FunctionType::FunctionDeclaration,
    Some(BindingIdentifier::new(SPAN, HELPER_NAME, &ast_builder)),
    false, // generator
    false, // async
    false, // declare
    NONE,  // type_parameters
    NONE,  // this_param
    params,
    NONE, // return_type
    Some(body),
    &ast_builder,
  ))
}
