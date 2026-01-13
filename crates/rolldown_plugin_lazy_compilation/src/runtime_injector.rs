use oxc::{
  allocator::Allocator,
  ast::{
    AstBuilder, NONE,
    ast::{Argument, Expression, FormalParameterKind, Statement},
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
      let import_expr = std::mem::replace(expr, self.ast_builder.expression_null_literal(SPAN));

      // Build: import_expr.then(__unwrap_lazy_compilation_entry)
      *expr = self.ast_builder.expression_call(
        SPAN,
        Expression::from(self.ast_builder.member_expression_static(
          SPAN,
          import_expr,
          self.ast_builder.identifier_name(SPAN, "then"),
          false,
        )),
        NONE,
        self
          .ast_builder
          .vec1(Argument::from(self.ast_builder.expression_identifier(SPAN, HELPER_NAME))),
        false,
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
  let params = ast_builder.formal_parameters(
    SPAN,
    FormalParameterKind::FormalParameter,
    ast_builder.vec1(ast_builder.formal_parameter(
      SPAN,
      ast_builder.vec(),
      ast_builder.binding_pattern_binding_identifier(SPAN, "m"),
      NONE,
      NONE,
      false,
      None,
      false,
      false,
    )),
    NONE,
  );

  // var e = m['rolldown:exports'];
  let var_decl_stmt = Statement::from(ast_builder.declaration_variable(
    SPAN,
    oxc::ast::ast::VariableDeclarationKind::Var,
    ast_builder.vec1(ast_builder.variable_declarator(
      SPAN,
      oxc::ast::ast::VariableDeclarationKind::Var,
      ast_builder.binding_pattern_binding_identifier(SPAN, "e"),
      NONE,
      Some(Expression::from(ast_builder.member_expression_computed(
        SPAN,
        ast_builder.expression_identifier(SPAN, "m"),
        ast_builder.expression_string_literal(SPAN, "rolldown:exports", None),
        false,
      ))),
      false,
    )),
    false,
  ));

  // return e ? e : m;
  let return_stmt = ast_builder.statement_return(
    SPAN,
    Some(ast_builder.expression_conditional(
      SPAN,
      ast_builder.expression_identifier(SPAN, "e"),
      ast_builder.expression_identifier(SPAN, "e"),
      ast_builder.expression_identifier(SPAN, "m"),
    )),
  );

  // Function body with both statements
  let mut body_stmts = ast_builder.vec_with_capacity(2);
  body_stmts.push(var_decl_stmt);
  body_stmts.push(return_stmt);
  let body = ast_builder.function_body(SPAN, ast_builder.vec(), body_stmts);

  // function __unwrap_lazy_compilation_entry(m) { ... }
  Statement::FunctionDeclaration(ast_builder.alloc_function(
    SPAN,
    oxc::ast::ast::FunctionType::FunctionDeclaration,
    Some(ast_builder.binding_identifier(SPAN, HELPER_NAME)),
    false, // generator
    false, // async
    false, // declare
    NONE,  // type_parameters
    NONE,  // this_param
    params,
    NONE, // return_type
    Some(body),
  ))
}
