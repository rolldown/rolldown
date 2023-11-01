use oxc::ast::Visit;

use super::RendererBase;

pub struct CjsRenderer<'ast> {
  base: RendererBase<'ast>,
}

impl<'ast> CjsRenderer<'ast> {
  pub fn new(base: RendererBase<'ast>) -> Self {
    Self { base }
  }

  pub fn apply(&mut self) {
    let program = self.base.module.ast.program();
    self.visit_program(program);
    let wrap_symbol_name = self.base.wrap_symbol_name.unwrap();
    let module_path = self.base.module.resource_id.prettify();
    let commonjs_runtime_symbol_name = self.base.canonical_name_for_runtime(&"__commonJS".into());
    self.base.source.prepend(format!(
      "var {wrap_symbol_name} = {commonjs_runtime_symbol_name}({{\n'{module_path}'(exports, module) {{\n",
    ));
    self.base.source.append("\n}\n});");
    assert!(!self.base.module.is_namespace_referenced());
  }
}

impl<'ast> Visit<'ast> for CjsRenderer<'ast> {
  fn visit_call_expression(&mut self, expr: &'ast oxc::ast::ast::CallExpression<'ast>) {
    self.base.visit_call_expression(expr);
    for arg in &expr.arguments {
      self.visit_argument(arg);
    }
    if let oxc::ast::ast::Expression::Identifier(s) = &expr.callee {
      self.base.visit_identifier_reference(s, true);
    } else {
      self.visit_expression(&expr.callee);
    }
  }

  fn visit_identifier_reference(&mut self, ident: &'ast oxc::ast::ast::IdentifierReference) {
    self.base.visit_identifier_reference(ident, false);
  }

  fn visit_import_declaration(&mut self, decl: &'ast oxc::ast::ast::ImportDeclaration<'ast>) {
    self.base.visit_import_declaration(decl);
  }

  fn visit_statement(&mut self, stmt: &'ast oxc::ast::ast::Statement<'ast>) {
    self.base.visit_statement(stmt);
    self.visit_statement_match(stmt);
  }
}
