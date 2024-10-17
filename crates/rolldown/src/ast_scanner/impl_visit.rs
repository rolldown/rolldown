use oxc::{
  ast::{
    ast::{self, Expression, IdentifierReference, MemberExpression},
    visit::walk,
    Visit,
  },
  span::{GetSpan, Span},
};
use rolldown_common::ImportKind;
use rolldown_ecmascript::ToSourceString;
use rolldown_error::BuildDiagnostic;
use rolldown_std_utils::OptionExt;

use crate::utils::call_expression_ext::CallExpressionExt;

use super::{side_effect_detector::SideEffectDetector, AstScanner};

impl<'me, 'ast> Visit<'ast> for AstScanner<'me> {
  fn visit_program(&mut self, program: &ast::Program<'ast>) {
    for (idx, stmt) in program.body.iter().enumerate() {
      self.current_stmt_info.stmt_idx = Some(idx);
      self.current_stmt_info.side_effect =
        SideEffectDetector::new(self.scopes, self.source, self.trivias)
          .detect_side_effect_of_stmt(stmt);

      if cfg!(debug_assertions) {
        self.current_stmt_info.debug_label = Some(stmt.to_source_string());
      }

      self.visit_statement(stmt);
      self.result.stmt_infos.add_stmt_info(std::mem::take(&mut self.current_stmt_info));
    }
  }

  fn visit_binding_identifier(&mut self, ident: &ast::BindingIdentifier) {
    let symbol_id = ident.symbol_id.get().unpack();
    if self.is_root_symbol(symbol_id) {
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
        let object_symbol_in_root_scope = loop {
          props_in_reverse_order.push(&cur_member_expr.property);
          match &cur_member_expr.object {
            Expression::StaticMemberExpression(expr) => {
              cur_member_expr = expr;
            }
            Expression::Identifier(id) => {
              break self.resolve_identifier_to_root_symbol(id);
            }
            _ => break None,
          }
        };
        match object_symbol_in_root_scope {
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
            // Don't walk again, otherwise we will add the `object_symbol_in_root_scope` again in `visit_identifier_reference`
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
    if let Some(root_symbol_id) = self.resolve_identifier_to_root_symbol(ident) {
      self.add_referenced_symbol(root_symbol_id);
    }
  }

  fn visit_statement(&mut self, stmt: &ast::Statement<'ast>) {
    if let Some(decl) = stmt.as_module_declaration() {
      self.scan_module_decl(decl);
    }
    walk::walk_statement(self, stmt);
  }

  fn visit_import_expression(&mut self, expr: &ast::ImportExpression<'ast>) {
    if let ast::Expression::StringLiteral(request) = &expr.source {
      let id = self.add_import_record(
        request.value.as_str(),
        ImportKind::DynamicImport,
        expr.source.span().start,
      );
      self.result.imports.insert(expr.span, id);
    }
    walk::walk_import_expression(self, expr);
  }

  fn visit_assignment_expression(&mut self, node: &ast::AssignmentExpression<'ast>) {
    match &node.left {
      ast::AssignmentTarget::AssignmentTargetIdentifier(id_ref) => {
        self.try_diagnostic_forbid_const_assign(id_ref);
      }
      // Detect `module.exports` and `exports.ANY`
      ast::AssignmentTarget::StaticMemberExpression(member_expr) => match member_expr.object {
        Expression::Identifier(ref id) => {
          if id.name == "module"
            && self.resolve_identifier_to_root_symbol(id).is_none()
            && member_expr.property.name == "exports"
          {
            self.cjs_module_ident.get_or_insert(Span::new(id.span.start, id.span.start + 6));
          }
          if id.name == "exports" && self.resolve_identifier_to_root_symbol(id).is_none() {
            self.cjs_exports_ident.get_or_insert(Span::new(id.span.start, id.span.start + 7));
          }
        }
        // `module.exports.test` is also considered as commonjs keyword
        Expression::StaticMemberExpression(ref member_expr) => {
          if let Expression::Identifier(ref id) = member_expr.object {
            if id.name == "module"
              && self.resolve_identifier_to_root_symbol(id).is_none()
              && member_expr.property.name == "exports"
            {
              self.cjs_module_ident.get_or_insert(Span::new(id.span.start, id.span.start + 6));
            }
          }
        }
        _ => {}
      },
      _ => {}
    }
    walk::walk_assignment_expression(self, node);
  }

  fn visit_call_expression(&mut self, expr: &ast::CallExpression<'ast>) {
    match &expr.callee {
      Expression::Identifier(id_ref) if id_ref.name == "eval" => {
        // TODO: esbuild track has_eval for each scope, this could reduce bailout range, and may
        // improve treeshaking performance. https://github.com/evanw/esbuild/blob/360d47230813e67d0312ad754cad2b6ee09b151b/internal/js_ast/js_ast.go#L1288-L1291
        if self.resolve_identifier_to_root_symbol(id_ref).is_none() {
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
      if let Some(ast::Argument::StringLiteral(request)) = &expr.arguments.first() {
        let id =
          self.add_import_record(request.value.as_str(), ImportKind::Require, request.span().start);
        self.result.imports.insert(expr.span, id);
      }
    }

    walk::walk_call_expression(self, expr);
  }
}
