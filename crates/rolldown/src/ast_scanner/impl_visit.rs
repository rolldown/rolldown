use oxc::{
  ast::{
    ast::{Expression, IdentifierReference, MemberExpression},
    visit::walk,
    Visit,
  },
  codegen::{self, CodeGenerator, Gen},
  span::GetSpan,
};
use rolldown_common::ImportKind;
use rolldown_error::BuildDiagnostic;

use crate::utils::call_expression_ext::CallExpressionExt;

use super::{side_effect_detector::SideEffectDetector, AstScanner};

impl<'me, 'ast> Visit<'ast> for AstScanner<'me> {
  fn visit_program(&mut self, program: &oxc::ast::ast::Program<'ast>) {
    for (idx, stmt) in program.body.iter().enumerate() {
      self.current_stmt_info.stmt_idx = Some(idx);
      self.current_stmt_info.side_effect =
        SideEffectDetector::new(self.scopes, self.source, self.trivias)
          .detect_side_effect_of_stmt(stmt);

      if cfg!(debug_assertions) {
        let mut codegen = CodeGenerator::new();
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
  }

  fn visit_member_expression(&mut self, expr: &MemberExpression<'ast>) {
    match expr {
      MemberExpression::StaticMemberExpression(member_expr) => {
        // For member expression like `a.b.c.d`, we will first enter the (object: `a.b.c`, property: `d`) expression.
        // So we add these properties with order `d`, `c`, `b`.
        let mut props_in_reverse_order = vec![];
        let mut cur_member_expr = member_expr;
        let object_symbol_in_top_level = loop {
          props_in_reverse_order.push(&cur_member_expr.property);
          match &cur_member_expr.object {
            Expression::StaticMemberExpression(expr) => {
              cur_member_expr = expr;
            }
            Expression::Identifier(id) => {
              break self.resolve_identifier_to_top_level_symbol(id);
            }
            _ => break None,
          }
        };
        match object_symbol_in_top_level {
          // - Import statements are hoisted to the top of the module, so in this time being, all imports are scanned.
          // - Having empty span will also results to bailout since we rely on span to identify ast nodes.
          Some(sym_ref)
            if self.result.named_imports.contains_key(&sym_ref) && !expr.span().is_unspanned() =>
          {
            let props = props_in_reverse_order
              .into_iter()
              .rev()
              .map(|ident| ident.name.as_str().into())
              .collect::<Vec<_>>();
            self.add_member_expr_reference(sym_ref, props, expr.span());
            // Don't walk again, otherwise we will add the `object_symbol_in_top_level` again in `visit_identifier_reference`
            return;
          }
          _ => {}
        }
      }
      _ => {}
    };
    walk::walk_member_expression(self, expr);
  }

  fn visit_identifier_reference(&mut self, ident: &IdentifierReference) {
    if let Some(top_level_symbol_id) = self.resolve_identifier_to_top_level_symbol(ident) {
      self.add_referenced_symbol(top_level_symbol_id);
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
      let id = self.add_import_record(
        request.value.as_str(),
        ImportKind::DynamicImport,
        expr.source.span().start,
      );
      self.result.imports.insert(expr.span, id);
    }
    walk::walk_import_expression(self, expr);
  }

  fn visit_assignment_expression(&mut self, node: &oxc::ast::ast::AssignmentExpression<'ast>) {
    match &node.left {
      oxc::ast::ast::AssignmentTarget::AssignmentTargetIdentifier(id_ref) => {
        self.try_diagnostic_forbid_const_assign(id_ref);
      }
      _ => {}
    }
    walk::walk_assignment_expression(self, node);
  }

  fn visit_call_expression(&mut self, expr: &oxc::ast::ast::CallExpression<'ast>) {
    match &expr.callee {
      Expression::Identifier(id_ref) if id_ref.name == "eval" => {
        // TODO: esbuild track has_eval for each scope, this could reduce bailout range, and may
        // improve treeshaking performance. https://github.com/evanw/esbuild/blob/360d47230813e67d0312ad754cad2b6ee09b151b/internal/js_ast/js_ast.go#L1288-L1291
        if self.resolve_identifier_to_top_level_symbol(id_ref).is_none() {
          self.result.has_eval = true;
          self.result.warnings.push(
            BuildDiagnostic::eval(self.file_path.to_string(), self.source.clone(), id_ref.span)
              .with_severity_warning(),
          );
        }
      }
      _ => {}
    }
    if expr.is_global_require_call(self.scopes) {
      if let Some(oxc::ast::ast::Argument::StringLiteral(request)) = &expr.arguments.first() {
        let id =
          self.add_import_record(request.value.as_str(), ImportKind::Require, request.span().start);
        self.result.imports.insert(expr.span, id);
      }
    }

    walk::walk_call_expression(self, expr);
  }
}
