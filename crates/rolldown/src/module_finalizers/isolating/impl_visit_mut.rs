use oxc::ast::ast::{self, Expression, Statement};
use oxc::ast::visit::walk_mut;
use oxc::ast::VisitMut;
use oxc::span::SPAN;
use rolldown_common::Module;
use rolldown_ecmascript::TakeIn;

use super::IsolatingModuleFinalizer;

impl<'me, 'ast> VisitMut<'ast> for IsolatingModuleFinalizer<'me, 'ast> {
  fn visit_program(&mut self, program: &mut ast::Program<'ast>) {
    let original_body = program.body.take_in(self.alloc);

    // Add __esModule flag for esm module
    if self.ctx.module.exports_kind.is_esm() {
      program.body.push(self.snippet.builder.statement_expression(
        SPAN,
        self.snippet.call_expr_with_arg_expr("__toCommonJS", "exports"),
      ));
    }

    for mut stmt in original_body {
      match &stmt {
        Statement::ImportDeclaration(import_decl) => {
          let rec_id = self.ctx.module.imports[&import_decl.span];
          let rec = &self.ctx.module.import_records[rec_id];
          match &self.ctx.modules[rec.resolved_module] {
            Module::Ecma(importee) => {
              if self.generated_imports.contains(&importee.namespace_object_ref) {
                continue;
              }
              // TODO deconflict namespace_ref
              let namespace_ref = self.ctx.symbols.get_original_name(importee.namespace_object_ref);

              self.generated_imports.insert(importee.namespace_object_ref);

              program.body.push(self.snippet.variable_declarator_require_call_stmt(
                import_decl.source.as_ref(),
                namespace_ref,
                import_decl.span,
              ));
              continue;
            }
            Module::External(_) => unimplemented!(),
          }
        }
        // TODO: rewrite `export default xxx` to `var __rolldown_default_export__ = xxx`
        ast::Statement::ExportDefaultDeclaration(_default_decl) => {}
        _ => {}
      }
      walk_mut::walk_statement(self, &mut stmt);
      program.body.push(stmt);
    }
  }

  fn visit_expression(&mut self, expr: &mut Expression<'ast>) {
    if let Expression::Identifier(ident) = expr {
      if let Some(named_import) = ident
        .reference_id
        .get()
        .and_then(|reference_id| self.scope.symbol_id_for(reference_id))
        .map(|symbol_id| (self.ctx.module.idx, symbol_id).into())
        .and_then(|symbol_ref| self.ctx.module.named_imports.get(&symbol_ref))
      {
        let rec = &self.ctx.module.import_records[named_import.record_id];
        match &self.ctx.modules[rec.resolved_module] {
          Module::Ecma(importee) => {
            // TODO deconflict namespace_ref
            let namespace_ref = self.ctx.symbols.get_original_name(importee.namespace_object_ref);

            match &named_import.imported {
              rolldown_common::Specifier::Star => {
                ident.name = self.snippet.atom(namespace_ref.as_str());
              }
              rolldown_common::Specifier::Literal(imported) => {
                *expr = Expression::StaticMemberExpression(
                  self.snippet.builder.alloc_static_member_expression(
                    ident.span,
                    self.snippet.id_ref_expr(namespace_ref, SPAN),
                    self.snippet.builder.identifier_name(SPAN, imported.as_str()),
                    false,
                  ),
                );
              }
            }
          }
          Module::External(_) => {}
        }
      };
    }
    walk_mut::walk_expression(self, expr);
  }
}
