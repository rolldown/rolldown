use oxc::{
  ast::ast::{self},
  span::SPAN,
};
use rolldown_common::{EcmaModuleAstUsage, ExportsKind, LocalExport, Module, StmtInfoIdx};
use rolldown_ecmascript::{AstSnippet, TakeIn};
use rolldown_utils::rayon::{IntoParallelRefMutIterator, ParallelIterator};
use rustc_hash::FxHashMap;

use super::LinkStage;

impl<'link> LinkStage<'link> {
  pub fn generate_lazy_export(&mut self) {
    let module_idx_to_exports_kind = append_only_vec::AppendOnlyVec::new();
    // let mut ast_table = std::mem::take(&mut self.ast_table);
    self.module_table.modules.par_iter_mut().for_each(|module| {
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
      module_idx_to_exports_kind.push((module.idx, module.exports_kind));

      // generate `module.exports = expr`
      if module.exports_kind == ExportsKind::CommonJs {
        // since the wrap arguments are generate on demand, we need to insert the module ref usage here.
        module.stmt_infos.infos[StmtInfoIdx::new(1)].side_effect = true;
        module.ecma_view.ast_usage.insert(EcmaModuleAstUsage::ModuleRef);
      }
    });

    let ast_idx_to_exports_kind =
      module_idx_to_exports_kind.into_iter().collect::<FxHashMap<_, _>>();
    self.ast_table.par_iter_mut().for_each(|(ecma_ast, idx)| {
      let Some(item) = ast_idx_to_exports_kind.get(idx) else {
        return;
      };
      if matches!(item, ExportsKind::CommonJs) {
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
  }
}
