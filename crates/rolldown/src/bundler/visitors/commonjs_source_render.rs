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
    let wrap_symbol_name = self.ctx.wrap_symbol_name.unwrap();
    let module_path = self.ctx.module.resource_id.prettify();
    let commonjs_runtime_symbol_name = self.ctx.get_runtime_symbol_final_name("__commonJS".into());
    self.ctx.source.prepend(format!(
      "var {wrap_symbol_name} = {commonjs_runtime_symbol_name}({{\n'{module_path}'(exports, module) {{\n",
    ));
    self.ctx.source.append("\n}\n});");
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
          let wrap_symbol_name = self
            .ctx
            .get_import_symbol_symbol_final_name(importee.wrap_symbol.unwrap());
          if importee.module_resolution == ModuleResolution::CommonJs {
            self.ctx.source.update(
              expr.span.start,
              expr.span.end,
              format!("{wrap_symbol_name}()"),
            );
          } else {
            let namespace_name = self
              .ctx
              .get_symbol_final_name((importee.id, importee.namespace_symbol.0.symbol).into())
              .unwrap();
            let to_commonjs_runtime_symbol_name = self
              .ctx
              .get_runtime_symbol_final_name("__toCommonJS".into());
            self.ctx.source.update(
              expr.span.start,
              expr.span.end,
              format!(
                "({wrap_symbol_name}(), {to_commonjs_runtime_symbol_name}({namespace_name}))"
              ),
            );
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
