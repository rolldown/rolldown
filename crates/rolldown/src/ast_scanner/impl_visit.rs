use std::sync::Arc;

use oxc::{
  ast::{ast::IdentifierReference, visit::walk, Visit},
  codegen::{self, Codegen, CodegenOptions, Gen},
};
use rolldown_common::ImportKind;
use rolldown_error::BuildError;

use super::{side_effect_detector::SideEffectDetector, AstScanner};

impl<'me, 'ast> Visit<'ast> for AstScanner<'me> {
  fn visit_program(&mut self, program: &oxc::ast::ast::Program<'ast>) {
    for (idx, stmt) in program.body.iter().enumerate() {
      self.current_stmt_info.stmt_idx = Some(idx);
      self.current_stmt_info.side_effect =
        SideEffectDetector::new(self.scope, self.source, self.trivias)
          .detect_side_effect_of_stmt(stmt);

      if cfg!(debug_assertions) {
        let mut codegen = Codegen::<false>::new(
          "",
          "",
          CodegenOptions { enable_typescript: true, enable_source_map: false },
        );
        stmt.gen(&mut codegen, codegen::Context::default());
        self.current_stmt_info.debug_label = Some(codegen.into_source_text());
      }

      self.visit_statement(stmt);
      self.result.stmt_infos.add_stmt_info(std::mem::take(&mut self.current_stmt_info));
    }
  }

  fn visit_binding_identifier(&mut self, ident: &oxc::ast::ast::BindingIdentifier) {
    let symbol_id = ident.symbol_id.get().unwrap();
    if self.is_top_level(symbol_id) {
      self.add_declared_id(symbol_id);
    }
    self.try_diagnostic_forbid_const_assign(symbol_id);
  }

  fn visit_identifier_reference(&mut self, ident: &IdentifierReference) {
    let symbol_id = self.resolve_symbol_from_reference(ident);
    match symbol_id {
      Some(symbol_id) if self.is_top_level(symbol_id) => {
        self.add_referenced_symbol(symbol_id);
      }
      None => {
        if ident.name == "module" {
          self.used_module_ref = true;
        }
        if ident.name == "exports" {
          self.used_exports_ref = true;
        }
        if ident.name == "eval" {
          self.result.warnings.push(
            BuildError::eval(self.file_path.to_string(), Arc::clone(self.source), ident.span)
              .with_severity_warning(),
          );
        }
      }
      _ => {}
    }
  }

  fn visit_statement(&mut self, stmt: &oxc::ast::ast::Statement<'ast>) {
    if let Some(decl) = stmt.as_module_declaration() {
      self.scan_module_decl(decl);
    }
    walk::walk_statement(self, stmt);
  }

  fn visit_import_expression(&mut self, expr: &oxc::ast::ast::ImportExpression<'ast>) {
    if let oxc::ast::ast::Expression::StringLiteral(request) = &expr.source {
      let id = self.add_import_record(&request.value, ImportKind::DynamicImport);
      self.result.imports.insert(expr.span, id);
    }
    walk::walk_import_expression(self, expr);
  }

  fn visit_call_expression(&mut self, expr: &oxc::ast::ast::CallExpression<'ast>) {
    match &expr.callee {
      oxc::ast::ast::Expression::Identifier(ident)
        if ident.name == "require" && self.is_unresolved_reference(ident) =>
      {
        if let Some(oxc::ast::ast::Argument::StringLiteral(request)) = &expr.arguments.first() {
          let id = self.add_import_record(&request.value, ImportKind::Require);
          self.result.imports.insert(expr.span, id);
        }
      }
      _ => {}
    }

    walk::walk_call_expression(self, expr);
  }
}
