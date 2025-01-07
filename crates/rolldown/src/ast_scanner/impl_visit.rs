use oxc::{
  ast::{
    ast::{self, BindingPatternKind, Expression, IdentifierReference},
    visit::walk,
    AstKind, Visit,
  },
  semantic::SymbolId,
  span::{GetSpan, Span},
};
use rolldown_common::{
  dynamic_import_usage::DynamicImportExportsUsage, generate_replace_this_expr_map,
  EcmaModuleAstUsage, ImportKind, ImportRecordMeta, StmtInfoMeta, ThisExprReplaceKind,
};
use rolldown_ecmascript::ToSourceString;
use rolldown_error::BuildDiagnostic;
use rolldown_std_utils::OptionExt;

use super::{
  esmodule_flag_analyzer::EsModuleFlagCheckType, side_effect_detector::SideEffectDetector,
  AstScanner,
};

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
      self.current_stmt_info.side_effect = SideEffectDetector::new(
        self.scopes,
        self.source,
        self.comments,
        // In `NormalModule` the options is always `Some`, for `RuntimeModule` always enable annotations
        !self.options.treeshake.annotations(),
        self.options.jsx.is_jsx_preserve(),
        &self.result.symbol_ref_db,
      )
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
    if self.result.has_eval {
      // if there exists `eval` in current module, assume all dynamic import are completely used;
      for usage in self.result.dynamic_import_rec_exports_usage.values_mut() {
        *usage = DynamicImportExportsUsage::Complete;
      }
    }

    // https://github.com/evanw/esbuild/blob/d34e79e2a998c21bb71d57b92b0017ca11756912/internal/js_parser/js_parser.go#L12551-L12604
    // Since AstScan is immutable, we defer transformation in module finalizer
    if !self.top_level_this_expr_set.is_empty() {
      if self.esm_export_keyword.is_none() {
        self.ast_usage.insert(EcmaModuleAstUsage::ExportsRef);
        self.result.this_expr_replace_map = generate_replace_this_expr_map(
          &self.top_level_this_expr_set,
          ThisExprReplaceKind::Exports,
        );
      } else {
        self.result.this_expr_replace_map = generate_replace_this_expr_map(
          &self.top_level_this_expr_set,
          ThisExprReplaceKind::Undefined,
        );
      }
    }
  }

  fn visit_binding_identifier(&mut self, ident: &ast::BindingIdentifier) {
    let symbol_id = ident.symbol_id.get().unpack();
    if self.is_root_symbol(symbol_id) {
      self.add_declared_id(symbol_id);
    }
  }

  fn visit_for_of_statement(&mut self, it: &ast::ForOfStatement<'ast>) {
    if it.r#await && self.is_top_level() && !self.options.format.keep_esm_import_export_syntax() {
      self.result.errors.push(BuildDiagnostic::unsupported_feature(
        self.id.resource_id().clone(),
        self.source.clone(),
        it.span(),
        format!(
          "Top-level await is currently not supported with the '{format}' output format",
          format = self.options.format
        ),
      ));
    }

    walk::walk_for_of_statement(self, it);
  }

  fn visit_await_expression(&mut self, it: &ast::AwaitExpression<'ast>) {
    if !self.options.format.keep_esm_import_export_syntax() && self.is_top_level() {
      self.result.errors.push(BuildDiagnostic::unsupported_feature(
        self.id.resource_id().clone(),
        self.source.clone(),
        it.span(),
        format!(
          "Top-level await is currently not supported with the '{format}' output format",
          format = self.options.format
        ),
      ));
    }
    walk::walk_await_expression(self, it);
  }

  fn visit_identifier_reference(&mut self, ident: &IdentifierReference) {
    self.process_identifier_ref_by_scope(ident);
    self.try_diagnostic_forbid_const_assign(ident);
    self.update_dynamic_import_binding_usage_info(ident);
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
      self.init_dynamic_import_binding_usage_info(import_rec_idx);
      self.result.imports.insert(expr.span, import_rec_idx);
    }
    walk::walk_import_expression(self, expr);
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

  fn visit_new_expression(&mut self, it: &ast::NewExpression<'ast>) {
    if self.options.experimental.is_resolve_new_url_to_asset_enabled() {
      self.handle_new_url_with_string_literal_and_import_meta_url(it);
    }
    walk::walk_new_expression(self, it);
  }

  fn visit_this_expression(&mut self, it: &ast::ThisExpression) {
    if !self.is_this_nested() {
      self.top_level_this_expr_set.insert(it.span);
    }
    walk::walk_this_expression(self, it);
  }

  fn visit_class(&mut self, it: &ast::Class<'ast>) {
    let previous_class_decl_id = self.cur_class_decl.take();
    self.cur_class_decl = self.get_class_id(it);
    walk::walk_class(self, it);
    self.cur_class_decl = previous_class_decl_id;
  }

  fn visit_class_element(&mut self, it: &ast::ClassElement<'ast>) {
    let pre_is_nested_this_inside_class = self.is_nested_this_inside_class;
    self.is_nested_this_inside_class = true;
    walk::walk_class_element(self, it);
    self.is_nested_this_inside_class = pre_is_nested_this_inside_class;
  }

  fn visit_property_key(&mut self, it: &ast::PropertyKey<'ast>) {
    let pre_is_nested_this_inside_class = self.is_nested_this_inside_class;
    self.is_nested_this_inside_class = false;
    walk::walk_property_key(self, it);
    self.is_nested_this_inside_class = pre_is_nested_this_inside_class;
  }

  fn visit_declaration(&mut self, it: &ast::Declaration<'ast>) {
    match it {
      ast::Declaration::VariableDeclaration(decl) => match decl.declarations.as_slice() {
        [decl] => {
          if let (BindingPatternKind::BindingIdentifier(_), Some(init)) =
            (&decl.id.kind, &decl.init)
          {
            match init {
              ast::Expression::ClassExpression(_) => {
                self.current_stmt_info.meta.insert(StmtInfoMeta::ClassExpr);
              }
              ast::Expression::FunctionExpression(_) => {
                self.current_stmt_info.meta.insert(StmtInfoMeta::FnExpr);
              }
              _ => {}
            }
          }
        }
        _ => {}
      },
      ast::Declaration::FunctionDeclaration(_) => {
        self.current_stmt_info.meta.insert(StmtInfoMeta::FnDecl);
      }
      ast::Declaration::ClassDeclaration(_) => {
        self.current_stmt_info.meta.insert(StmtInfoMeta::ClassDecl);
      }
      _ => {}
    }
    walk::walk_declaration(self, it);
  }
}

impl<'me, 'ast: 'me> AstScanner<'me, 'ast> {
  /// visit `Class` of declaration
  #[allow(clippy::unused_self)]
  pub fn get_class_id(&mut self, class: &ast::Class<'ast>) -> Option<SymbolId> {
    let id = class.id.as_ref()?;
    let symbol_id = *id.symbol_id.get().unpack_ref();
    Some(symbol_id)
  }

  fn process_identifier_ref_by_scope(&mut self, ident_ref: &IdentifierReference) {
    match self.resolve_identifier_reference(ident_ref) {
      super::IdentifierReferenceKind::Global => {
        if !self.ast_usage.contains(EcmaModuleAstUsage::ModuleOrExports) {
          match ident_ref.name.as_str() {
            "module" => {
              if self
                .check_es_module_flag(&EsModuleFlagCheckType::ModuleExportsAssignment)
                .unwrap_or_default()
              {
                self.ast_usage.insert(EcmaModuleAstUsage::EsModuleFlag);
              };
              self.ast_usage.insert(EcmaModuleAstUsage::ModuleRef);
            }
            "exports" => {
              if self
                .check_es_module_flag(&EsModuleFlagCheckType::ExportsAssignment)
                .unwrap_or_default()
              {
                self.ast_usage.insert(EcmaModuleAstUsage::EsModuleFlag);
              };
              self.ast_usage.insert(EcmaModuleAstUsage::ExportsRef);
            }
            _ => {}
          }
        }
        self.process_global_identifier_ref_by_ancestor(ident_ref);
      }
      super::IdentifierReferenceKind::Root(root_symbol_id) => {
        // if the identifier_reference is a NamedImport MemberExpr access, we store it as a `MemberExpr`
        // use this flag to avoid insert it as `Symbol` at the same time.
        let mut is_inserted_before = false;
        if self.result.named_imports.contains_key(&root_symbol_id) {
          if let Some((span, props)) = self.try_extract_parent_static_member_expr_chain(usize::MAX)
          {
            if !span.is_unspanned() {
              is_inserted_before = true;
              self.add_member_expr_reference(root_symbol_id, props, span);
            }
          }
        }
        if !is_inserted_before {
          self.add_referenced_symbol(root_symbol_id);
        }

        self.check_import_assign(ident_ref, root_symbol_id.symbol);

        match (self.cur_class_decl, self.resolve_symbol_from_reference(ident_ref)) {
          (Some(cur_class_decl), Some(referenced_to)) if cur_class_decl == referenced_to => {
            self.result.self_referenced_class_decl_symbol_ids.insert(cur_class_decl);
          }
          _ => {}
        }
      }
      super::IdentifierReferenceKind::Other => {}
    };
  }

  fn process_global_identifier_ref_by_ancestor(
    &mut self,
    ident_ref: &IdentifierReference,
  ) -> Option<()> {
    let parent = self.visit_path.last()?;
    match parent {
      AstKind::CallExpression(call_expr) => {
        match ident_ref.name.as_str() {
          "eval" => {
            // TODO: esbuild track has_eval for each scope, this could reduce bailout range, and may
            // improve treeshaking performance. https://github.com/evanw/esbuild/blob/360d47230813e67d0312ad754cad2b6ee09b151b/internal/js_ast/js_ast.go#L1288-L1291
            self.result.has_eval = true;
            self.result.warnings.push(
              BuildDiagnostic::eval(self.id.to_string(), self.source.clone(), ident_ref.span)
                .with_severity_warning(),
            );
          }
          "require" => {
            self.process_global_require_call(call_expr);
          }
          _ => {}
        }
      }
      _ => {}
    }
    None
  }

  fn process_global_require_call(&mut self, expr: &ast::CallExpression<'ast>) {
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
          // skip one for CallExpression it self
          for ancestor in self.visit_path.iter().rev().skip(1) {
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
}
