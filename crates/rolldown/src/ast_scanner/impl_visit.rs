use oxc::{
  ast::{
    AstKind,
    ast::{self, BindingPatternKind, Declaration, Expression, IdentifierReference},
  },
  ast_visit::{Visit, walk},
  semantic::{ScopeFlags, SymbolId},
  span::{GetSpan, Span},
};
use rolldown_common::{
  ConstExportMeta, EcmaModuleAstUsage, EcmaViewMeta, ImportKind, ImportRecordMeta, LocalExport,
  RUNTIME_MODULE_KEY, SideEffectDetail, StmtInfoMeta, SymbolRefFlags,
  dynamic_import_usage::DynamicImportExportsUsage,
};
#[cfg(debug_assertions)]
use rolldown_ecmascript::ToSourceString;
use rolldown_ecmascript_utils::{ExpressionExt, is_top_level};
use rolldown_error::BuildDiagnostic;
use rolldown_std_utils::OptionExt;

use crate::ast_scanner::{TraverseState, cjs_ast_analyzer::CommonJsAstType};

use super::{
  AstScanner, cjs_ast_analyzer::CjsGlobalAssignmentType, side_effect_detector::SideEffectDetector,
};

impl<'me, 'ast: 'me> Visit<'ast> for AstScanner<'me, 'ast> {
  fn enter_scope(
    &mut self,
    flags: oxc::semantic::ScopeFlags,
    _scope_id: &std::cell::Cell<Option<oxc::semantic::ScopeId>>,
  ) {
    self.scope_stack.push(flags);
    self.traverse_state.set(TraverseState::TopLevel, is_top_level(&self.scope_stack));
  }

  fn leave_scope(&mut self) {
    self.scope_stack.pop();
    self.traverse_state.set(TraverseState::TopLevel, is_top_level(&self.scope_stack));
  }

  fn enter_node(&mut self, kind: oxc::ast::AstKind<'ast>) {
    self.visit_path.push(kind);
  }

  fn leave_node(&mut self, _: oxc::ast::AstKind<'ast>) {
    self.visit_path.pop();
  }

  fn visit_simple_assignment_target(&mut self, it: &ast::SimpleAssignmentTarget<'ast>) {
    if !self.immutable_ctx.flat_options.property_write_side_effects()
      && self.traverse_state.contains(TraverseState::TopLevel)
    {
      match it {
        ast::SimpleAssignmentTarget::ComputedMemberExpression(_)
        | ast::SimpleAssignmentTarget::StaticMemberExpression(_) => {
          let pre = self.traverse_state;
          self.traverse_state.insert(TraverseState::RootSymbolReferenceStmtInfoId);
          walk::walk_simple_assignment_target(self, it);
          self.traverse_state = pre;
          return;
        }
        _ => {}
      }
    }
    walk::walk_simple_assignment_target(self, it);
  }

  fn visit_program(&mut self, program: &ast::Program<'ast>) {
    self.enter_scope(
      {
        let mut flags = ScopeFlags::Top;
        if program.source_type.is_strict() || program.has_use_strict_directive() {
          flags |= ScopeFlags::StrictMode;
        }
        flags
      },
      &program.scope_id,
    );
    // Custom visit
    for (idx, stmt) in program.body.iter().enumerate() {
      self.current_stmt_info.stmt_idx = Some(idx.into());
      self.current_stmt_info.side_effect = SideEffectDetector::new(
        &self.result.symbol_ref_db.ast_scopes,
        self.immutable_ctx.flat_options,
        self.immutable_ctx.options,
        None,
      )
      .detect_side_effect_of_stmt(stmt);

      #[cfg(debug_assertions)]
      {
        self.current_stmt_info.debug_label = Some(stmt.to_source_string());
      }

      self.visit_statement(stmt);
      if self.current_stmt_info.side_effect.intersects(
        SideEffectDetail::Unknown
          | SideEffectDetail::GlobalVarAccess
          | SideEffectDetail::PureAnnotation,
      ) {
        self.result.ecma_view_meta.insert(EcmaViewMeta::ExecutionOrderSensitive);
      }
      self.result.stmt_infos.add_stmt_info(std::mem::take(&mut self.current_stmt_info));
    }

    self.result.hashbang_range = program.hashbang.as_ref().map(GetSpan::span);
    self.result.directive_range = program.directives.iter().map(GetSpan::span).collect();
    self.result.dynamic_import_rec_exports_usage =
      std::mem::take(&mut self.dynamic_import_usage_info.dynamic_import_exports_usage);
    if self.result.ecma_view_meta.contains(EcmaViewMeta::Eval) {
      // if there exists `eval` in current module, assume all dynamic import are completely used;
      for usage in self.result.dynamic_import_rec_exports_usage.values_mut() {
        *usage = DynamicImportExportsUsage::Complete;
      }
    }

    // Check if dynamic import record is a pure dynamic import
    for (rec_idx, usage) in &self.result.dynamic_import_rec_exports_usage {
      if matches!(usage, DynamicImportExportsUsage::Partial(set) if set.is_empty()) {
        self.result.import_records[*rec_idx].meta.insert(ImportRecordMeta::PureDynamicImport);
      }
    }

    // check if the module is a reexport cjs module e.g.
    // module.exports = require('a');
    // normalize ast usage flag
    if self.result.ast_usage.contains(EcmaModuleAstUsage::ModuleRef)
      || !self.result.ast_usage.contains(EcmaModuleAstUsage::ExportsRef)
    {
      self.result.ast_usage.remove(EcmaModuleAstUsage::AllStaticExportPropertyAccess);
    }
    self.leave_scope();
  }

  fn visit_binding_identifier(&mut self, ident: &ast::BindingIdentifier) {
    let symbol_id = ident.symbol_id.get().unpack();
    if self.is_root_symbol(symbol_id) {
      self.declare_normal_symbol_ref(symbol_id);
    }
  }

  fn visit_for_of_statement(&mut self, it: &ast::ForOfStatement<'ast>) {
    let is_top_level_await = it.r#await && self.is_valid_tla_scope();
    if is_top_level_await && !self.immutable_ctx.flat_options.keep_esm_import_export_syntax() {
      self.result.errors.push(BuildDiagnostic::unsupported_feature(
        self.immutable_ctx.id.resource_id().clone(),
        self.immutable_ctx.source.clone(),
        it.span(),
        format!(
          "Top-level await is currently not supported with the '{format}' output format",
          format = self.immutable_ctx.options.format
        ),
      ));
    }
    if is_top_level_await {
      self.result.ast_usage.insert(EcmaModuleAstUsage::TopLevelAwait);
    }

    walk::walk_for_of_statement(self, it);
  }

  fn visit_await_expression(&mut self, it: &ast::AwaitExpression<'ast>) {
    let is_top_level_await = self.is_valid_tla_scope();
    if !self.immutable_ctx.flat_options.keep_esm_import_export_syntax() && is_top_level_await {
      self.result.errors.push(BuildDiagnostic::unsupported_feature(
        self.immutable_ctx.id.resource_id().clone(),
        self.immutable_ctx.source.clone(),
        it.span(),
        format!(
          "Top-level await is currently not supported with the '{format}' output format",
          format = self.immutable_ctx.options.format
        ),
      ));
    }
    if is_top_level_await {
      self.result.ast_usage.insert(EcmaModuleAstUsage::TopLevelAwait);
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

  fn visit_return_statement(&mut self, stmt: &ast::ReturnStatement<'ast>) {
    // Top-level return statements are only valid in CommonJS modules
    if self.traverse_state.contains(TraverseState::TopLevel) {
      self.result.ast_usage.insert(EcmaModuleAstUsage::TopLevelReturn);
    }
    walk::walk_return_statement(self, stmt);
  }

  fn visit_import_expression(&mut self, expr: &ast::ImportExpression<'ast>) {
    if let Some(request) = expr.source.as_static_module_request() {
      let import_rec_idx =
        self.add_import_record(request.as_str(), ImportKind::DynamicImport, expr.source.span(), {
          let mut meta = ImportRecordMeta::empty();
          meta.set(ImportRecordMeta::IsTopLevel, self.is_root_scope());
          meta.set(ImportRecordMeta::IsUnspannedImport, expr.source.span().is_empty());
          meta.set(ImportRecordMeta::InTryCatchBlock, self.in_side_try_catch_block());
          meta
        });
      self.init_dynamic_import_binding_usage_info(import_rec_idx);
      self.result.imports.insert(expr.span, import_rec_idx);
    }
    walk::walk_import_expression(self, expr);
  }

  fn visit_assignment_expression(&mut self, node: &ast::AssignmentExpression<'ast>) {
    match node.left.as_member_expression() {
      Some(member_expr) => {
        match member_expr.object() {
          Expression::Identifier(id) => {
            if id.name == "module"
              && self.is_global_identifier_reference(id)
              && member_expr.static_property_name() == Some("exports")
            {
              self.cjs_module_ident.get_or_insert(Span::new(id.span.start, id.span.start + 6));
            }
            if id.name == "exports" && self.is_global_identifier_reference(id) {
              self.cjs_exports_ident.get_or_insert(Span::new(id.span.start, id.span.start + 7));

              if let Some((span, export_name)) = member_expr.static_property_info() {
                // `exports.test = ...`
                let exported_symbol =
                  self.result.symbol_ref_db.create_facade_root_symbol_ref(export_name);

                self.declare_link_only_symbol_ref(exported_symbol.symbol);

                if let Some(value) = self.extract_constant_value_from_expr(Some(&node.right)) {
                  self
                    .add_constant_symbol(exported_symbol.symbol, ConstExportMeta::new(value, true));
                }

                self.result.commonjs_exports.insert(
                  export_name.into(),
                  LocalExport { referenced: exported_symbol, span, came_from_commonjs: true },
                );
              }
            }
          }
          // `module.exports.test` is also considered as commonjs keyword
          Expression::StaticMemberExpression(member_expr) => {
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
        }
      }
      None => {}
    }

    walk::walk_assignment_expression(self, node);
  }

  fn visit_new_expression(&mut self, it: &ast::NewExpression<'ast>) {
    if self.immutable_ctx.flat_options.resolve_new_url_to_asset_enabled() {
      self.handle_new_url_with_string_literal_and_import_meta_url(it);
    }
    walk::walk_new_expression(self, it);
  }

  fn visit_meta_property(&mut self, it: &ast::MetaProperty<'ast>) {
    if self.immutable_ctx.flat_options.keep_esm_import_export_syntax() {
      walk::walk_meta_property(self, it);
      return;
    }
    if let Some(parent) = self.visit_path.last() {
      if !parent
        .as_member_expression_kind()
        .map(|member_expr| {
          let static_name = member_expr.static_property_name().unwrap_or(ast::Atom::from(""));
          static_name == "url" || static_name == "dirname" || static_name == "filename"
        })
        // Here we need to set it to `false` to emit warnings when leaving `import.meta` alone along with the logic `not` head of this.
        .unwrap_or(false)
        && it.meta.name == "import"
        && it.property.name == "meta"
      {
        self.result.warnings.push(
          BuildDiagnostic::empty_import_meta(
            self
              .immutable_ctx
              .id
              .resource_id()
              .clone()
              .parse()
              .expect("should be a valid resource id"),
            self.immutable_ctx.source.clone(),
            it.span(),
            self
              .immutable_ctx
              .options
              .format
              .to_string()
              .parse()
              .expect("should be a valid format"),
          )
          .with_severity_warning(),
        );
      }
    }
  }

  fn visit_this_expression(&mut self, it: &ast::ThisExpression) {
    if !self.is_this_nested() {
      self.top_level_this_expr_set.insert(it.span);
    }
    walk::walk_this_expression(self, it);
  }

  fn visit_class_element(&mut self, it: &ast::ClassElement<'ast>) {
    let pre_is_nested_this_inside_class = self.is_nested_this_inside_class;
    self.is_nested_this_inside_class = true;
    walk::walk_class_element(self, it);
    self.is_nested_this_inside_class = pre_is_nested_this_inside_class;
  }

  fn visit_property_key(&mut self, it: &ast::PropertyKey<'ast>) {
    let pre_is_nested_this_inside_class = self.is_nested_this_inside_class;
    if let Some(AstKind::ClassBody(_)) = self.visit_path.iter().rev().nth(1) {
      self.is_nested_this_inside_class = false;
    }
    walk::walk_property_key(self, it);
    self.is_nested_this_inside_class = pre_is_nested_this_inside_class;
  }

  fn visit_variable_declaration(&mut self, decl: &ast::VariableDeclaration<'ast>) {
    match decl.declarations.as_slice() {
      [decl] => {
        if let (BindingPatternKind::BindingIdentifier(binding), Some(init)) =
          (&decl.id.kind, &decl.init)
        {
          match init {
            ast::Expression::ClassExpression(_) => {
              self.current_stmt_info.meta.insert(StmtInfoMeta::ClassExpr);
            }
            ast::Expression::FunctionExpression(_func) => {
              self.current_stmt_info.meta.insert(StmtInfoMeta::FnExpr);
            }
            _ => {}
          }

          // Extract constant value for top-level variable declarations
          if self.is_root_symbol(binding.symbol_id()) {
            if let Some(value) = self.extract_constant_value_from_expr(Some(init)) {
              self.add_constant_symbol(binding.symbol_id(), ConstExportMeta::new(value, false));
            }
          }
        }
      }
      _ => {
        if self.immutable_ctx.flat_options.inline_const_enabled() && self.is_root_scope() {
          for var_decl in &decl.declarations {
            if let BindingPatternKind::BindingIdentifier(binding) = &var_decl.id.kind {
              if let Some(init) = &var_decl.init {
                if let Some(value) = self.extract_constant_value_from_expr(Some(init)) {
                  self.add_constant_symbol(binding.symbol_id(), ConstExportMeta::new(value, false));
                }
              }
            }
          }
        }
        // Handle multiple declarations in a single statement
      }
    }
    walk::walk_variable_declaration(self, decl);
  }

  fn visit_declaration(&mut self, it: &ast::Declaration<'ast>) {
    match it {
      Declaration::VariableDeclaration(_) => {
        walk::walk_declaration(self, it);
      }
      Declaration::FunctionDeclaration(function) => {
        self.visit_function_decl(function, ScopeFlags::Function);
      }
      Declaration::ClassDeclaration(class) => {
        self.visit_class_decl(class);
      }

      Declaration::TSTypeAliasDeclaration(_)
      | Declaration::TSInterfaceDeclaration(_)
      | Declaration::TSEnumDeclaration(_)
      | Declaration::TSModuleDeclaration(_)
      | Declaration::TSImportEqualsDeclaration(_) => unreachable!(),
    }
  }

  fn visit_call_expression(&mut self, it: &ast::CallExpression<'ast>) {
    self.try_extract_hmr_info_from_hot_accept_call(it);
    walk::walk_call_expression(self, it);
  }
}

impl<'me, 'ast: 'me> AstScanner<'me, 'ast> {
  /// visit `Class` of declaration
  #[expect(clippy::unused_self)]
  pub fn get_class_id(&self, class: &ast::Class<'ast>) -> Option<SymbolId> {
    let id = class.id.as_ref()?;
    let symbol_id = *id.symbol_id.get().unpack_ref();
    Some(symbol_id)
  }

  fn process_identifier_ref_by_scope(&mut self, ident_ref: &IdentifierReference) {
    match self.resolve_identifier_reference(ident_ref) {
      super::IdentifierReferenceKind::Global => {
        match ident_ref.name.as_str() {
          "module" => {
            self.cjs_ast_analyzer(&CjsGlobalAssignmentType::ModuleExportsAssignment);
          }
          "exports" => {
            // exports = {} will not change the module.exports object, so we just ignore it;
            let v = self.cjs_ast_analyzer(&CjsGlobalAssignmentType::ExportsAssignment);
            match v {
              // Do nothing since we need to tree shake `exports.<prop>` access
              Some(CommonJsAstType::ExportsPropWrite | CommonJsAstType::EsModuleFlag) => {}
              Some(CommonJsAstType::Reexport) => {
                // This is only usd for `module.exports = require('mod')`
                // should only reached when `ident_ref` is `exports`
                unreachable!()
              }
              Some(CommonJsAstType::ExportsRead) => {
                self.result.ast_usage.insert(EcmaModuleAstUsage::UnknownExportsRead);
              }
              None => match self.try_extract_parent_static_member_expr_chain(1) {
                Some((_span, prop)) => {
                  self.self_used_cjs_named_exports.insert(prop[0].0.clone());
                }
                _ => {
                  self.result.ast_usage.insert(EcmaModuleAstUsage::UnknownExportsRead);
                }
              },
            }
          }
          "require" => {
            let is_dummy_record = match self.visit_path.last() {
              Some(AstKind::CallExpression(call_expr)) => {
                !self.process_global_require_call(call_expr)
              }
              Some(_) => true,
              _ => false,
            };
            // should not replace require in `runtime` code
            if is_dummy_record
              && self.immutable_ctx.id.as_ref() != RUNTIME_MODULE_KEY
              && self.immutable_ctx.flat_options.should_call_runtime_require()
              && self
                .immutable_ctx
                .flat_options
                .polyfill_require_for_esm_format_with_node_platform()
            {
              self.current_stmt_info.meta.insert(StmtInfoMeta::HasDummyRecord);
              self.result.dummy_record_set.insert(ident_ref.span);
            }
          }
          _ => {}
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

        if self.traverse_state.contains(TraverseState::RootSymbolReferenceStmtInfoId) {
          // Since `0` is always namespace object stmt info
          self.result.stmt_infos.reference_stmt_for_symbol_id(
            self.current_stmt_info.stmt_idx.unwrap() + 1,
            root_symbol_id,
          );
        }

        self.check_import_assign(ident_ref, root_symbol_id.symbol);

        match (self.cur_class_decl, self.resolve_symbol_from_reference(ident_ref)) {
          (Some(cur_class_decl), Some(referenced_to)) if cur_class_decl == referenced_to => {
            self.result.self_referenced_class_decl_symbol_ids.insert(cur_class_decl);
          }
          _ => {}
        }

        if self.immutable_ctx.flat_options.jsx_preserve()
          && self.visit_path.last().is_some_and(|ast_kind| {
            matches!(ast_kind, AstKind::JSXOpeningElement(_) | AstKind::JSXClosingElement(_))
          })
        {
          let symbol_ref_flags = root_symbol_id.flags_mut(&mut self.result.symbol_ref_db);
          *symbol_ref_flags |= SymbolRefFlags::MustStartWithCapitalLetterForJSX;
        }
      }
      super::IdentifierReferenceKind::Other => {}
    }
  }

  fn process_global_identifier_ref_by_ancestor(
    &mut self,
    ident_ref: &IdentifierReference,
  ) -> Option<()> {
    let parent = self.visit_path.last()?;
    if let AstKind::CallExpression(_) = parent {
      if ident_ref.name == "eval" {
        // TODO: esbuild track has_eval for each scope, this could reduce bailout range, and may
        // improve treeshaking performance. https://github.com/evanw/esbuild/blob/360d47230813e67d0312ad754cad2b6ee09b151b/internal/js_ast/js_ast.go#L1288-L1291
        self.result.ecma_view_meta.insert(EcmaViewMeta::Eval);
        self.result.warnings.push(
          BuildDiagnostic::eval(
            self.immutable_ctx.id.to_string(),
            self.immutable_ctx.source.clone(),
            ident_ref.span,
          )
          .with_severity_warning(),
        );
      }
    }
    None
  }

  /// return `bool` represent if it is a global require call
  fn process_global_require_call(&mut self, expr: &ast::CallExpression<'ast>) -> bool {
    let (value, span) = match expr.arguments.first() {
      Some(ast::Argument::StringLiteral(request)) => (request.value, request.span),
      Some(ast::Argument::TemplateLiteral(request)) => match request.single_quasi() {
        Some(value) => (value, request.span),
        None => return false,
      },
      _ => return false,
    };
    let mut init_meta = if span.is_empty() {
      ImportRecordMeta::IsUnspannedImport
    } else {
      let mut is_require_used = true;
      let mut meta = ImportRecordMeta::empty();
      // traverse nearest ExpressionStatement and check if there are potential used
      // skip one for CallExpression it self
      for ancestor in self.visit_path.iter().rev().skip(1) {
        match ancestor {
          AstKind::ParenthesizedExpression(_) => {}
          AstKind::ExpressionStatement(_) => {
            meta.insert(ImportRecordMeta::IsRequireUnused);
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
    };
    let in_side_try_catch_block = self.in_side_try_catch_block();
    init_meta.set(ImportRecordMeta::InTryCatchBlock, in_side_try_catch_block);
    let id = self.add_import_record(value.as_ref(), ImportKind::Require, span, init_meta);
    self.result.imports.insert(expr.span, id);
    true
  }
}
