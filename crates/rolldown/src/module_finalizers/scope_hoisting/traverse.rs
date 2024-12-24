use oxc::{
  allocator::{self, IntoIn},
  ast::{
    ast::{self, Expression},
    match_member_expression,
  },
  span::SPAN,
};
use oxc_traverse::Traverse;
use rolldown_common::{ExportsKind, Module, StmtInfoIdx, SymbolRef, ThisExprReplaceKind, WrapKind};
use rolldown_ecmascript_utils::TakeIn;

use super::ScopeHoistingFinalizer;

impl<'ast, 'me> Traverse<'ast> for ScopeHoistingFinalizer<'me, 'ast> {
  fn enter_program(
    &mut self,
    program: &mut ast::Program<'ast>,
    _ctx: &mut oxc_traverse::TraverseCtx<'ast>,
  ) {
    // Drop the hashbang since we already store them in ast_scan phase and
    // we don't want oxc to generate hashbang statement in module level since we already handle
    // them in chunk level
    program.hashbang.take();

    self.remove_unused_top_level_stmt(program);
  }

  fn exit_program(
    &mut self,
    program: &mut ast::Program<'ast>,
    _ctx: &mut oxc_traverse::TraverseCtx<'ast>,
  ) {
    let is_namespace_referenced = matches!(self.ctx.module.exports_kind, ExportsKind::Esm)
      && self.ctx.module.stmt_infos[StmtInfoIdx::new(0)].is_included;
    // check if we need to add wrapper
    let needs_wrapper = self
      .ctx
      .linking_info
      .wrapper_stmt_info
      .is_some_and(|idx| self.ctx.module.stmt_infos[idx].is_included);

    // the order should be
    // 1. module namespace object declaration
    // 2. shimmed_exports
    // 3. hoisted_names
    // 4. wrapped module declaration
    let declaration_of_module_namespace_object = if is_namespace_referenced {
      let stmts = self.generate_declaration_of_module_namespace_object();
      if needs_wrapper {
        stmts
      } else {
        program.body.splice(0..0, stmts);
        vec![]
      }
    } else {
      vec![]
    };

    let mut shimmed_exports =
      self.ctx.linking_info.shimmed_missing_exports.iter().collect::<Vec<_>>();
    shimmed_exports.sort_unstable_by_key(|(name, _)| name.as_str());
    shimmed_exports.into_iter().for_each(|(_name, symbol_ref)| {
      debug_assert!(!self.ctx.module.stmt_infos.declared_stmts_by_symbol(symbol_ref).is_empty());
      let is_included: bool = self
        .ctx
        .module
        .stmt_infos
        .declared_stmts_by_symbol(symbol_ref)
        .iter()
        .any(|id| self.ctx.module.stmt_infos[*id].is_included);
      if is_included {
        let canonical_name = self.canonical_name_for(*symbol_ref);
        program.body.push(self.snippet.var_decl_stmt(canonical_name, self.snippet.void_zero()));
      }
    });
    if needs_wrapper {
      match self.ctx.linking_info.wrap_kind {
        WrapKind::Cjs => {
          let wrap_ref_name = self.canonical_name_for(self.ctx.linking_info.wrapper_ref.unwrap());
          let commonjs_ref = if self.ctx.options.profiler_names {
            self.canonical_ref_for_runtime("__commonJS")
          } else {
            self.canonical_ref_for_runtime("__commonJSMin")
          };

          let commonjs_ref_expr = self.finalized_expr_for_symbol_ref(commonjs_ref, false);

          let old_body = program.body.take_in(self.alloc);

          program.body.push(self.snippet.commonjs_wrapper_stmt(
            wrap_ref_name,
            commonjs_ref_expr,
            old_body,
            self.ctx.module.ast_usage,
            self.ctx.options.profiler_names,
            &self.ctx.module.stable_id,
          ));
        }
        WrapKind::Esm => {
          use ast::Statement;
          let wrap_ref_name = self.canonical_name_for(self.ctx.linking_info.wrapper_ref.unwrap());
          let esm_ref = if self.ctx.options.profiler_names {
            self.canonical_ref_for_runtime("__esm")
          } else {
            self.canonical_ref_for_runtime("__esmMin")
          };
          let esm_ref_expr = self.finalized_expr_for_symbol_ref(esm_ref, false);
          let old_body = program.body.take_in(self.alloc);

          let mut fn_stmts = allocator::Vec::new_in(self.alloc);
          let mut hoisted_names = vec![];
          let mut stmts_inside_closure = allocator::Vec::new_in(self.alloc);

          // Hoist all top-level "var" and "function" declarations out of the closure
          old_body.into_iter().for_each(|mut stmt| match &mut stmt {
            ast::Statement::VariableDeclaration(_) => {
              if let Some(converted) =
                self.convert_decl_to_assignment(stmt.to_declaration_mut(), &mut hoisted_names)
              {
                stmts_inside_closure.push(converted);
              }
            }
            ast::Statement::FunctionDeclaration(_) => {
              fn_stmts.push(stmt);
            }
            ast::match_module_declaration!(Statement) => {
              if stmt.is_typescript_syntax() {
                unreachable!(
                  "At this point, typescript module declarations should have been removed or transformed"
                )
              }
              program.body.push(stmt);
            }
            _ => {
              stmts_inside_closure.push(stmt);
            }
          });
          program.body.extend(declaration_of_module_namespace_object);
          program.body.extend(fn_stmts);
          if !hoisted_names.is_empty() {
            let mut declarators = allocator::Vec::new_in(self.alloc);
            declarators.reserve_exact(hoisted_names.len());
            hoisted_names.into_iter().for_each(|var_name| {
              declarators.push(ast::VariableDeclarator {
                id: ast::BindingPattern {
                  kind: ast::BindingPatternKind::BindingIdentifier(
                    self.snippet.id(&var_name, SPAN).into_in(self.alloc),
                  ),
                  ..TakeIn::dummy(self.alloc)
                },
                kind: ast::VariableDeclarationKind::Var,
                ..TakeIn::dummy(self.alloc)
              });
            });
            program.body.push(ast::Statement::VariableDeclaration(
              ast::VariableDeclaration {
                declarations: declarators,
                kind: ast::VariableDeclarationKind::Var,
                ..TakeIn::dummy(self.alloc)
              }
              .into_in(self.alloc),
            ));
          }
          program.body.push(self.snippet.esm_wrapper_stmt(
            wrap_ref_name,
            esm_ref_expr,
            stmts_inside_closure,
            self.ctx.options.profiler_names,
            &self.ctx.module.stable_id,
          ));
        }
        WrapKind::None => {}
      }
    } else {
      program.body.extend(declaration_of_module_namespace_object);
    }
  }

  fn enter_binding_identifier(
    &mut self,
    ident: &mut ast::BindingIdentifier<'ast>,
    _ctx: &mut oxc_traverse::TraverseCtx<'ast>,
  ) {
    if let Some(symbol_id) = ident.symbol_id.get() {
      let symbol_ref: SymbolRef = (self.ctx.id, symbol_id).into();

      let canonical_ref = self.ctx.symbol_db.canonical_ref_for(symbol_ref);
      let symbol = self.ctx.symbol_db.get(canonical_ref);
      assert!(symbol.namespace_alias.is_none());
      let canonical_name = self.canonical_name_for(symbol_ref);
      if ident.name != canonical_name.as_str() {
        ident.name = self.snippet.atom(canonical_name);
      }
      ident.symbol_id.get_mut().take();
    } else {
      // Some `BindingIdentifier`s constructed by bundler don't have `SymbolId` and we just ignore them.
    }
  }

  fn enter_statement(
    &mut self,
    it: &mut ast::Statement<'ast>,
    _ctx: &mut oxc_traverse::TraverseCtx<'ast>,
  ) {
    if !self.ctx.options.drop_labels.is_empty() {
      match it {
        ast::Statement::LabeledStatement(stmt)
          if self.ctx.options.drop_labels.contains(stmt.label.name.as_str()) =>
        {
          self.snippet.builder.move_statement(it);
        }
        _ => {}
      }
    }
  }

  fn enter_identifier_reference(
    &mut self,
    ident: &mut ast::IdentifierReference<'ast>,
    _ctx: &mut oxc_traverse::TraverseCtx<'ast>,
  ) {
    // This ensure all `IdentifierReference`s are processed
    debug_assert!(
      self.is_global_identifier_reference(ident) || ident.reference_id.get().is_none(),
      "{} doesn't get processed in {}",
      ident.name,
      self.ctx.module.repr_name
    );
  }

  fn enter_expression(
    &mut self,
    expr: &mut ast::Expression<'ast>,
    ctx: &mut oxc_traverse::TraverseCtx<'ast>,
  ) {
    let node = ctx.parent();
    match expr {
      ast::Expression::CallExpression(call_expr) => {
        if let Some(new_expr) = self.try_rewrite_global_require_call(call_expr) {
          *expr = new_expr;
        }
      }
      // inline dynamic import
      ast::Expression::ImportExpression(import_expr) => {
        if let Some(new_expr) = self.try_rewrite_inline_dynamic_import_expr(import_expr) {
          *expr = new_expr;
        }
      }
      ast::Expression::NewExpression(new_expr) => {
        self.handle_new_url_with_string_literal_and_import_meta_url(new_expr);
      }
      ast::Expression::Identifier(ident_ref) => {
        if let Some(new_expr) = self.try_rewrite_identifier_reference_expr(ident_ref, false) {
          *expr = new_expr;
        }
      }
      ast::Expression::ThisExpression(this_expr) => {
        if let Some(kind) = self.ctx.module.ecma_view.this_expr_replace_map.get(&this_expr.span) {
          match kind {
            ThisExprReplaceKind::Exports => {
              *expr = self.snippet.builder.expression_identifier_reference(SPAN, "exports");
            }
            ThisExprReplaceKind::Undefined => {
              *expr = self.snippet.void_zero();
            }
          }
        }
      }
      _ => {
        if let Some(new_expr) =
          expr.as_member_expression().and_then(|expr| self.try_rewrite_member_expr(expr))
        {
          *expr = new_expr;
        }
      }
    };
  }

  fn exit_member_expression(
    &mut self,
    expr: &mut ast::MemberExpression<'ast>,
    _ctx: &mut oxc_traverse::TraverseCtx<'ast>,
  ) {
    if let Some(new_expr) = self.try_rewrite_member_expr(expr) {
      match new_expr {
        match_member_expression!(Expression) => {
          *expr = new_expr.into_member_expression();
        }
        _ => {
          unreachable!("Always rewrite to MemberExpression for nested MemberExpression")
        }
      }
    }
  }

  fn enter_object_property(
    &mut self,
    prop: &mut ast::ObjectProperty<'ast>,
    ctx: &mut oxc_traverse::TraverseCtx<'ast>,
  ) {
    // Ensure `{ a }` would be rewritten to `{ a: a$1 }` instead of `{ a$1 }`
    match &mut prop.value {
      ast::Expression::Identifier(id_ref) if prop.shorthand => {
        if let Some(expr) = self.generate_finalized_expr_for_reference(id_ref, false) {
          prop.value = expr;
          prop.shorthand = false;
        } else {
          id_ref.reference_id.get_mut().take();
        }
      }
      _ => {}
    }
  }

  fn enter_object_pattern(
    &mut self,
    pat: &mut ast::ObjectPattern<'ast>,
    ctx: &mut oxc_traverse::TraverseCtx<'ast>,
  ) {
    self.rewrite_object_pat_shorthand(pat);
  }

  fn enter_import_expression(
    &mut self,
    expr: &mut ast::ImportExpression<'ast>,
    ctx: &mut oxc_traverse::TraverseCtx<'ast>,
  ) {
    // Make sure the import expression is in correct form. If it's not, we should leave it as it is.
    match &mut expr.source {
      ast::Expression::StringLiteral(str) if expr.arguments.len() == 0 => {
        let rec_id = self.ctx.module.imports[&expr.span];
        let rec = &self.ctx.module.import_records[rec_id];
        let importee_id = rec.resolved_module;
        match &self.ctx.modules[importee_id] {
          Module::Normal(_importee) => {
            let importer_chunk_id = self.ctx.chunk_graph.module_to_chunk[self.ctx.module.idx]
              .expect("Normal module should belong to a chunk");
            let importer_chunk = &self.ctx.chunk_graph.chunk_table[importer_chunk_id];

            let importee_chunk_id = self.ctx.chunk_graph.entry_module_to_entry_chunk[&importee_id];
            let importee_chunk = &self.ctx.chunk_graph.chunk_table[importee_chunk_id];

            let import_path = importer_chunk.import_path_for(importee_chunk);

            str.value = self.snippet.atom(&import_path);
          }
          Module::External(importee) => {
            if str.value != importee.name {
              str.value = self.snippet.atom(&importee.name);
            }
          }
        }
      }
      _ => {}
    }
  }
}
