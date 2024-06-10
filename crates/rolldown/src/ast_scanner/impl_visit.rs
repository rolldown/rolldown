use std::sync::Arc;

use oxc::{
  ast::{
    ast::{Expression, IdentifierReference, MemberExpression},
    visit::walk,
    Visit,
  },
  codegen::{self, Codegen, CodegenOptions, Gen},
  semantic::SymbolId,
};
use rolldown_common::ImportKind;
use rolldown_error::BuildError;

use crate::utils::call_expression_ext::CallExpressionExt;

use super::{side_effect_detector::SideEffectDetector, AstScanner};

impl<'me> AstScanner<'me> {
  /// resolve the symbol from the identifier reference, and return if it is a top level symbol
  fn resolve_identifier_reference(
    &mut self,
    symbol_id: Option<SymbolId>,
    ident: &IdentifierReference,
  ) -> Option<SymbolId> {
    match symbol_id {
      Some(symbol_id) if self.is_top_level(symbol_id) => Some(symbol_id),
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
        None
      }
      _ => None,
    }
  }
}

impl<'me, 'ast> Visit<'ast> for AstScanner<'me> {
  fn visit_program(&mut self, program: &oxc::ast::ast::Program<'ast>) {
    for (idx, stmt) in program.body.iter().enumerate() {
      self.current_stmt_info.stmt_idx = Some(idx);
      self.current_stmt_info.side_effect =
        SideEffectDetector::new(self.scopes, self.source, self.trivias)
          .detect_side_effect_of_stmt(stmt);

      if cfg!(debug_assertions) {
        let mut codegen = Codegen::<false>::new(
          "",
          "",
          CodegenOptions {
            enable_typescript: true,
            enable_source_map: false,
            preserve_annotate_comments: false,
          },
          None,
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

  fn visit_member_expression(&mut self, expr: &MemberExpression<'ast>) {
    let top_level_member_expr = match expr {
      MemberExpression::ComputedMemberExpression(expr) => {
        self.visit_computed_member_expression(expr);
        None
      }
      MemberExpression::StaticMemberExpression(inner_expr) => {
        let mut chain = vec![];
        let mut cur = inner_expr;
        let top_level_symbol = loop {
          chain.push(cur.property.clone());
          match &cur.object {
            Expression::StaticMemberExpression(expr) => {
              cur = expr;
            }
            Expression::Identifier(ident) => {
              let symbol_id = self.resolve_symbol_from_reference(ident);
              let resolved_top_level = self.resolve_identifier_reference(symbol_id, ident);
              break resolved_top_level;
            }
            _ => break None,
          }
        };
        chain.reverse();
        let chain = chain.into_iter().map(|ident| ident.name.as_str().into()).collect::<Vec<_>>();
        if let Some(symbol_id) = top_level_symbol {
          Some((symbol_id, chain))
        } else {
          self.visit_expression(&cur.object);
          None
        }
      }
      MemberExpression::PrivateFieldExpression(expr) => {
        self.visit_private_field_expression(expr);
        None
      }
    };
    if let Some((symbol_id, chains)) = top_level_member_expr {
      self.add_member_expr_reference(symbol_id, chains);
    }
  }

  fn visit_identifier_reference(&mut self, ident: &IdentifierReference) {
    let symbol_id = self.resolve_symbol_from_reference(ident);
    if let Some(resolved_symbol_id) = self.resolve_identifier_reference(symbol_id, ident) {
      self.add_referenced_symbol(resolved_symbol_id);
    };
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
    if expr.is_global_require_call(self.scopes) {
      if let Some(oxc::ast::ast::Argument::StringLiteral(request)) = &expr.arguments.first() {
        let id = self.add_import_record(&request.value, ImportKind::Require);
        self.result.imports.insert(expr.span, id);
      }
    }

    walk::walk_call_expression(self, expr);
  }
}
