use oxc::ast::Visit;

use super::RendererContext;

pub struct CommonJsSourceRender<'ast> {
  ctx: RendererContext<'ast>,
}

impl<'ast> CommonJsSourceRender<'ast> {
  pub fn new(ctx: RendererContext<'ast>) -> Self {
    Self { ctx }
  }

  pub fn apply(&mut self) {
    let program = self.ctx.module.ast.program();
    self.visit_program(program);
    let wrap_symbol_name = self.ctx.wrap_symbol_name.unwrap();
    let module_path = self.ctx.module.resource_id.prettify();
    let commonjs_runtime_symbol_name = self.ctx.get_runtime_symbol_final_name(&"__commonJS".into());
    self.ctx.source.prepend(format!(
      "var {wrap_symbol_name} = {commonjs_runtime_symbol_name}({{\n'{module_path}'(exports, module) {{\n",
    ));
    self.ctx.source.append("\n}\n});");
    if let Some(s) = self.ctx.generate_namespace_variable_declaration() {
      self.ctx.source.prepend(s);
    }
  }
}

impl<'ast> Visit<'ast> for CommonJsSourceRender<'ast> {
  fn visit_call_expression(&mut self, expr: &'ast oxc::ast::ast::CallExpression<'ast>) {
    self.ctx.visit_call_expression(expr);
    for arg in &expr.arguments {
      self.visit_argument(arg);
    }
    if let oxc::ast::ast::Expression::Identifier(s) = &expr.callee {
      self.ctx.visit_identifier_reference(s, true);
    } else {
      self.visit_expression(&expr.callee);
    }
  }

  fn visit_identifier_reference(&mut self, ident: &'ast oxc::ast::ast::IdentifierReference) {
    self.ctx.visit_identifier_reference(ident, false);
  }

  fn visit_import_declaration(&mut self, decl: &'ast oxc::ast::ast::ImportDeclaration<'ast>) {
    self.ctx.visit_import_declaration(decl);
  }

  fn visit_statement(&mut self, stmt: &'ast oxc::ast::ast::Statement<'ast>) {
    self.ctx.visit_statement(stmt);
    self.visit_statement_match(stmt);
  }
}
