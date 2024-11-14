use oxc::{
  ast::{
    ast::{self, Expression, IdentifierReference},
    visit::walk,
    AstKind, Visit,
  },
  span::{GetSpan, Span},
};
use rolldown_common::{
  dynamic_import_usage::DynamicImportExportsUsage, ImportKind, ImportRecordMeta,
};
use rolldown_ecmascript::ToSourceString;
use rolldown_error::BuildDiagnostic;
use rolldown_std_utils::OptionExt;

use crate::utils::call_expression_ext::CallExpressionExt;

use super::{side_effect_detector::SideEffectDetector, AstScanner};

impl<'me, 'ast: 'me> Visit<'ast> for AstScanner<'me, 'ast> {
  fn enter_scope(
    &mut self,
    _flags: oxc::semantic::ScopeFlags,
    scope_id: &std::cell::Cell<Option<oxc::semantic::ScopeId>>,
  ) {
    self.scope_stack.push(scope_id.get());
  }

  fn leave_scope(&mut self) {
    self.scope_stack.pop();
  }

  fn enter_node(&mut self, kind: oxc::ast::AstKind<'ast>) {
    self.visit_path.push(kind);
  }

  fn leave_node(&mut self, _: oxc::ast::AstKind<'ast>) {
    self.visit_path.pop();
  }

  fn visit_program(&mut self, program: &ast::Program<'ast>) {
    for (idx, stmt) in program.body.iter().enumerate() {
      self.current_stmt_info.stmt_idx = Some(idx);
      self.current_stmt_info.side_effect =
        SideEffectDetector::new(self.scopes, self.source, self.comments)
          .detect_side_effect_of_stmt(stmt);

      if cfg!(debug_assertions) {
        self.current_stmt_info.debug_label = Some(stmt.to_source_string());
      }

      self.visit_statement(stmt);
      self.result.stmt_infos.add_stmt_info(std::mem::take(&mut self.current_stmt_info));
    }
    self.result.hashbang_range = program.hashbang.as_ref().map(GetSpan::span);
    self.result.dynamic_import_rec_exports_usage =
      std::mem::take(&mut self.dynamic_import_usage_info.dynamic_import_exports_usage);
  }

  fn visit_binding_identifier(&mut self, ident: &ast::BindingIdentifier) {
    let symbol_id = ident.symbol_id.get().unpack();
    if self.is_root_symbol(symbol_id) {
      self.add_declared_id(symbol_id);
    }
  }

  fn visit_for_of_statement(&mut self, it: &ast::ForOfStatement<'ast>) {
    if it.r#await && self.is_top_level() {
      if let Some(format) = self.options.as_ref().map(|option| &option.format) {
        if !format.keep_esm_import_export_syntax() {
          self.result.errors.push(BuildDiagnostic::unsupported_feature(
            self.file_path.as_str().into(),
            self.source.clone(),
            it.span(),
            format!(
              "Top-level await is currently not supported with the '{format}' output format",
            ),
          ));
        }
      }
    }

    walk::walk_for_of_statement(self, it);
  }

  fn visit_await_expression(&mut self, it: &ast::AwaitExpression<'ast>) {
    if let Some(format) = self.options.as_ref().map(|option| &option.format) {
      if !format.keep_esm_import_export_syntax() && self.is_top_level() {
        self.result.errors.push(BuildDiagnostic::unsupported_feature(
          self.file_path.as_str().into(),
          self.source.clone(),
          it.span(),
          format!("Top-level await is currently not supported with the '{format}' output format",),
        ));
      }
    }
    walk::walk_await_expression(self, it);
  }

  fn visit_identifier_reference(&mut self, ident: &IdentifierReference) {
    if let Some(root_symbol_id) = self.resolve_identifier_to_root_symbol(ident) {
      // if the identifier_reference is a NamedImport MemberExpr access, we store it as a `MemberExpr`
      // use this flag to avoid insert it as `Symbol` at the same time.
      let mut is_inserted_before = false;
      if self.result.named_imports.contains_key(&root_symbol_id) {
        if let Some((span, props)) = self.try_extract_parent_static_member_expr_chain(usize::MAX) {
          if !span.is_unspanned() {
            is_inserted_before = true;
            self.add_member_expr_reference(root_symbol_id, props, span);
          }
        }
      }
      if !is_inserted_before {
        self.add_referenced_symbol(root_symbol_id);
      }
      self.check_import_assign(ident, root_symbol_id.symbol);
    }
    if let Some((symbol_id, ids)) = self.cur_class_decl_and_symbol_referenced_ids {
      if ids.contains(&ident.reference_id()) {
        self.result.self_referenced_class_decl_symbol_ids.insert(symbol_id);
      }
    }
    _ = self.try_diagnostic_forbid_const_assign(ident);
    _ = self.update_dynamic_import_binding_usage_info(ident);
  }

  fn visit_statement(&mut self, stmt: &ast::Statement<'ast>) {
    if let Some(decl) = stmt.as_module_declaration() {
      self.scan_module_decl(decl);
    }
    walk::walk_statement(self, stmt);
  }

  fn visit_import_expression(&mut self, expr: &ast::ImportExpression<'ast>) {
    if let ast::Expression::StringLiteral(request) = &expr.source {
      let import_rec_idx = self.add_import_record(
        request.value.as_str(),
        ImportKind::DynamicImport,
        expr.source.span(),
        if expr.source.span().is_empty() {
          ImportRecordMeta::IS_UNSPANNED_IMPORT
        } else {
          ImportRecordMeta::empty()
        },
      );
      match self.init_dynamic_import_binding_usage_info(import_rec_idx) {
        Some(set) => {
          self
            .dynamic_import_usage_info
            .dynamic_import_exports_usage
            .insert(import_rec_idx, DynamicImportExportsUsage::Partial(set));
        }
        None => {
          self
            .dynamic_import_usage_info
            .dynamic_import_exports_usage
            .insert(import_rec_idx, DynamicImportExportsUsage::Complete);
        }
      };
      self.result.imports.insert(expr.span, import_rec_idx);
    }
    walk::walk_import_expression(self, expr);
  }

  fn visit_declaration(&mut self, it: &ast::Declaration<'ast>) {
    if let ast::Declaration::ClassDeclaration(class) = it {
      self.scan_class_declaration(class);
    }
    walk::walk_declaration(self, it);
  }

  fn visit_assignment_expression(&mut self, node: &ast::AssignmentExpression<'ast>) {
    match &node.left {
      // Detect `module.exports` and `exports.ANY`
      ast::AssignmentTarget::StaticMemberExpression(member_expr) => match member_expr.object {
        Expression::Identifier(ref id) => {
          if id.name == "module"
            && self.is_global_identifier_reference(id)
            && member_expr.property.name == "exports"
          {
            self.cjs_module_ident.get_or_insert(Span::new(id.span.start, id.span.start + 6));
          }
          if id.name == "exports" && self.is_global_identifier_reference(id) {
            self.cjs_exports_ident.get_or_insert(Span::new(id.span.start, id.span.start + 7));
          }
        }
        // `module.exports.test` is also considered as commonjs keyword
        Expression::StaticMemberExpression(ref member_expr) => {
          if let Expression::Identifier(ref id) = member_expr.object {
            if id.name == "module"
              && self.is_global_identifier_reference(id)
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
        let id = self.add_import_record(
          request.value.as_str(),
          ImportKind::Require,
          request.span(),
          if request.span().is_empty() {
            ImportRecordMeta::IS_UNSPANNED_IMPORT
          } else {
            let mut is_require_used = true;
            let mut meta = ImportRecordMeta::empty();
            // traverse nearest ExpressionStatement and check if there are potential used
            for ancestor in self.visit_path.iter().rev() {
              match ancestor {
                AstKind::ParenthesizedExpression(_) => {}
                AstKind::ExpressionStatement(_) => {
                  meta.insert(ImportRecordMeta::IS_REQUIRE_UNUSED);
                  break;
                }
                AstKind::SequenceExpression(seq_expr) => {
                  // the child node has require and it is potential used
                  // the state may changed according to the child node position
                  // 1. `1, 2, (1, require('a'))` => since the last child contains `require`, and
                  //    in the last position, it is still used if it meant any other astKind
                  // 2. `1, 2, (1, require('a')), 1` => since the last child contains `require`, but it is
                  //    not in the last position, the state should change to unused
                  let last = seq_expr.expressions.last().expect("should have at least one child");

                  if !last.span().is_empty() && !expr.span.is_empty() {
                    is_require_used = last.span().contains_inclusive(expr.span);
                  } else {
                    is_require_used = true;
                  }
                }
                _ => {
                  if is_require_used {
                    break;
                  }
                }
              }
            }
            meta
          },
        );
        self.result.imports.insert(expr.span, id);
      }
    }

    walk::walk_call_expression(self, expr);
  }

  fn visit_new_expression(&mut self, it: &ast::NewExpression<'ast>) {
    self.handle_new_url_with_string_literal_and_import_meta_url(it);
    walk::walk_new_expression(self, it);
  }
}

impl<'me, 'ast: 'me> AstScanner<'me, 'ast> {
  /// visit `Class` of declaration
  pub fn scan_class_declaration(&mut self, class: &ast::Class<'ast>) {
    let Some(id) = class.id.as_ref() else {
      return;
    };
    let symbol_id = *id.symbol_id.get().unpack_ref();
    let previous_reference_id = self.cur_class_decl_and_symbol_referenced_ids.take();
    self.cur_class_decl_and_symbol_referenced_ids =
      Some((symbol_id, &self.scopes.resolved_references[symbol_id]));
    walk::walk_class(self, class);
    self.cur_class_decl_and_symbol_referenced_ids = previous_reference_id;
  }
}
