use oxc::{
  ast::{ast::Expression, AstBuilder},
  codegen::CodeGenerator,
  parser::Parser,
  span::SourceType,
};

// A workaround to clone an expression AST.
// TODO: Remove this function once `Expression` can be cloned.
pub fn clone_expr<'ast>(
  ast_builder: AstBuilder<'ast>,
  expr: &Expression<'ast>,
) -> Expression<'ast> {
  let a = ast_builder.allocator;
  let mut g = CodeGenerator::new();
  g.print_expression(expr);
  let s = g.into_source_text();
  let s = a.alloc_str(s.as_str());
  Parser::new(a, s, SourceType::default()).parse_expression().unwrap()
}
