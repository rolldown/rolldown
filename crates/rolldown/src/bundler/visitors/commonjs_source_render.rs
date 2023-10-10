use oxc::ast::Visit;
use rolldown_common::ModuleResolution;

use crate::bundler::module::module::Module;

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
    if let Some(wrap_symbol_name) = self.ctx.get_wrap_symbol_name() {
      let module_path = self.ctx.module.resource_id.prettify();
      self.ctx.source.prepend(format!(
        "var {wrap_symbol_name} = __commonJS({{\n'{module_path}'(exports, module) {{\n",
      ));
      self.ctx.source.append("\n}\n});");
    }
  }
}

impl<'ast> Visit<'ast> for CommonJsSourceRender<'ast> {
  fn visit_call_expression(&mut self, expr: &'ast oxc::ast::ast::CallExpression<'ast>) {
    if let oxc::ast::ast::Expression::Identifier(ident) = &expr.callee {
      if ident.name == "require" {
        let rec = &self.ctx.module.import_records
          [self.ctx.module.imports.get(&expr.span).copied().unwrap()];
        let importee = &self.ctx.modules[rec.resolved_module];
        if let Module::Normal(importee) = importee {
          if let Some(wrap_symbol_name) = self.ctx.get_wrap_symbol_name() {
            if importee.module_resolution == ModuleResolution::CommonJs {
              self.ctx.source.update(
                expr.span.start,
                expr.span.end,
                format!("{wrap_symbol_name}()"),
              );
            } else if let Some(namespace_name) = self.ctx.get_namespace_symbol_name() {
              self.ctx.source.update(
                expr.span.start,
                expr.span.end,
                format!("({wrap_symbol_name}(), __toCommonJS({namespace_name}))"),
              );
            }
          }
        }
      }
    }
    for arg in expr.arguments.iter() {
      self.visit_argument(arg);
    }
    self.visit_expression(&expr.callee);
  }
}
