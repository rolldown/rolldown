use futures::StreamExt;
use oxc::{
  ast::{
    ast::{self, ExportDefaultDeclaration},
    AstBuilder,
  },
  span::SPAN,
};
use rolldown_common::{EcmaModuleAstUsage, ExportsKind, LocalExport, Module, StmtInfoIdx};
use rolldown_ecmascript::TakeIn;

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
      dbg!(&module.exports_kind);
      let default_symbol_ref = module.default_export_ref;
      module
        .named_exports
        .insert("default".into(), LocalExport { span: SPAN, referenced: default_symbol_ref });
      module.stmt_infos.declare_symbol_for_stmt(1.into(), default_symbol_ref);
      module.stmt_infos.infos[StmtInfoIdx::new(1)].side_effect = true;
      let (ecma_ast, _) = &mut self.ast_table[module.ecma_ast_idx()];
      // if module.stable_id.ends_with(".txt") {
      //   dbg!(&ecma_ast.program());
      // }
      // generate `module.exports = expr`
      if module.exports_kind == ExportsKind::CommonJs {
        module.ecma_view.ast_usage.insert(EcmaModuleAstUsage::ModuleRef);
        ecma_ast.program.with_mut(|fields| {
          let ast_builder = AstBuilder::new(fields.allocator);
          let Some(item) = fields.program.body.first_mut() else { unreachable!() };
          match item {
            ast::Statement::ExpressionStatement(stmt) => {
              let expr = stmt.expression.take_in(ast_builder.allocator);
              *stmt = ast_builder.alloc_expression_statement(
                SPAN,
                ast_builder.expression_assignment(
                  SPAN,
                  ast::AssignmentOperator::Assign,
                  ast_builder.assignment_target_simple(
                    ast_builder.simple_assignment_target_member_expression(
                      ast_builder.member_expression_static(
                        SPAN,
                        ast_builder.expression_identifier_reference(SPAN, "module"),
                        ast_builder.identifier_name(SPAN, "exports"),
                        false,
                      ),
                    ),
                  ),
                  expr,
                ),
              );
            }
            _ => {
              unreachable!()
            }
          }
        });
        return;
      }

      ecma_ast.program.with_mut(|fields| {
        let ast_builder = AstBuilder::new(fields.allocator);
        let Some(item) = fields.program.body.first_mut() else { unreachable!() };
        let expr = match item {
          ast::Statement::ExpressionStatement(stmt) => {
            let expr = stmt.expression.take_in(ast_builder.allocator);
            expr
          }
          _ => {
            unreachable!()
          }
        };
        *item = ast_builder.statement_module_declaration(
          ast_builder.module_declaration_export_default_declaration(
            SPAN,
            ast_builder.export_default_declaration_kind_expression(expr),
            ast_builder.module_export_name_identifier_name(SPAN, "default"),
          ),
        );
      });
    });
    // This is safe since there is no two module mutate the same ast;
  }
}
