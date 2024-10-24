use oxc::{
  ast::ast::{self},
  span::SPAN,
};
use rolldown_common::{EcmaModuleAstUsage, ExportsKind, LocalExport, Module, StmtInfoIdx};
use rolldown_ecmascript::{AstSnippet, TakeIn};

use super::LinkStage;

impl<'link> LinkStage<'link> {
  pub fn generate_lazy_export(&mut self) {
    // let mut ast_table = std::mem::take(&mut self.ast_table);
    self.module_table.modules.iter_mut().for_each(|module| {
      let Module::Normal(module) = module else {
        return;
      };
      if !module.has_lazy_export {
        return;
      }
      let default_symbol_ref = module.default_export_ref;
      module
        .named_exports
        .insert("default".into(), LocalExport { span: SPAN, referenced: default_symbol_ref });
      module.stmt_infos.declare_symbol_for_stmt(1.into(), default_symbol_ref);
      module.stmt_infos.infos[StmtInfoIdx::new(1)].side_effect = true;
      let (ecma_ast, _) = &mut self.ast_table[module.ecma_ast_idx()];

      // generate `module.exports = expr`
      if module.exports_kind == ExportsKind::CommonJs {
        // since the wrap arguments are generate on demand, we need to insert the module ref usage here.
        module.ecma_view.ast_usage.insert(EcmaModuleAstUsage::ModuleRef);
        ecma_ast.program.with_mut(|fields| {
          let snippet = AstSnippet::new(fields.allocator);
          let Some(stmt) = fields.program.body.first_mut() else { unreachable!() };
          let expr = match stmt {
            ast::Statement::ExpressionStatement(stmt) => stmt.expression.take_in(snippet.alloc()),
            _ => {
              unreachable!()
            }
          };
          *stmt = snippet.module_exports_expr_stmt(expr);
        });
        return;
      }

      // TODO: json

      ecma_ast.program.with_mut(|fields| {
        let snippet = AstSnippet::new(fields.allocator);
        let Some(stmt) = fields.program.body.first_mut() else { unreachable!() };
        let expr = match stmt {
          ast::Statement::ExpressionStatement(stmt) => stmt.expression.take_in(snippet.alloc()),
          _ => {
            unreachable!()
          }
        };
        *stmt = snippet.export_default_expr_stmt(expr);
      });
    });
    // This is safe since there is no two module mutate the same ast;
  }
}
