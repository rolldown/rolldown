// cSpell:disable

use oxc::{
  allocator::{self, IntoIn},
  ast::{
    ast::{
      self, BindingIdentifier, BindingPatternKind, ClassElement, Expression, ImportExpression,
      SimpleAssignmentTarget, VariableDeclarationKind,
    },
    match_member_expression,
    visit::walk_mut,
    VisitMut, NONE,
  },
  span::{GetSpan, Span, SPAN},
};
use rolldown_common::{
  ExportsKind, ImportRecordMeta, Module, ModuleType, OutputFormat, StmtInfoIdx, SymbolRef,
  ThisExprReplaceKind, WrapKind,
};
use rolldown_ecmascript_utils::{AllocatorExt, ExpressionExt, StatementExt, TakeIn};
use rolldown_rstr::Rstr;

use crate::utils::call_expression_ext::CallExpressionExt;

use super::ScopeHoistingFinalizer;

impl<'ast> VisitMut<'ast> for ScopeHoistingFinalizer<'_, 'ast> {
  #[allow(clippy::too_many_lines)]
  fn visit_program(&mut self, program: &mut ast::Program<'ast>) {
    // Drop the hashbang since we already store them in ast_scan phase and
    // we don't want oxc to generate hashbang statement in module level since we already handle
    // them in chunk level
    program.hashbang.take();

    let is_namespace_referenced = matches!(self.ctx.module.exports_kind, ExportsKind::Esm)
      && self.ctx.module.stmt_infos[StmtInfoIdx::new(0)].is_included;

    self.remove_unused_top_level_stmt(program);

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
    walk_mut::walk_program(self, program);

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

  fn visit_binding_identifier(&mut self, ident: &mut ast::BindingIdentifier<'ast>) {
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

  fn visit_statement(&mut self, it: &mut ast::Statement<'ast>) {
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
    walk_mut::walk_statement(self, it);
  }

  fn visit_statements(&mut self, it: &mut allocator::Vec<'ast, ast::Statement<'ast>>) {
    let previous_stmt_index = self.ctx.cur_stmt_index;
    let previous_keep_name_statement = std::mem::take(&mut self.ctx.keep_name_statement_to_insert);
    for (i, stmt) in it.iter_mut().enumerate() {
      self.ctx.cur_stmt_index = i;
      self.visit_statement(stmt);
    }

    // TODO: perf it
    for (stmt_index, _symbol_id, original_name, new_name) in
      self.ctx.keep_name_statement_to_insert.iter().rev()
    {
      it.insert(*stmt_index, self.snippet.keep_name_call_expr_stmt(original_name, new_name));
    }
    self.ctx.cur_stmt_index = previous_stmt_index;
    self.ctx.keep_name_statement_to_insert = previous_keep_name_statement;
  }

  fn visit_identifier_reference(&mut self, ident: &mut ast::IdentifierReference) {
    // This ensure all `IdentifierReference`s are processed
    debug_assert!(
      self.is_global_identifier_reference(ident) || ident.reference_id.get().is_none(),
      "{} doesn't get processed in {}",
      ident.name,
      self.ctx.module.repr_name
    );
  }

  fn visit_call_expression(&mut self, expr: &mut ast::CallExpression<'ast>) {
    if let Some(new_expr) = expr
      .callee
      .as_identifier_mut()
      .and_then(|ident_ref| self.try_rewrite_identifier_reference_expr(ident_ref, true))
    {
      expr.callee = new_expr;
    }

    walk_mut::walk_call_expression(self, expr);
  }

  fn visit_expression(&mut self, expr: &mut ast::Expression<'ast>) {
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

    walk_mut::walk_expression(self, expr);
  }

  // foo.js `export const bar = { a: 0 }`
  // main.js `import * as foo_exports from './foo.js';\n foo_exports.bar.a = 1;`
  // The `foo_exports.bar.a` ast is `StaticMemberExpression(StaticMemberExpression)`, The outer StaticMemberExpression span is `foo_exports.bar.a`, the `visit_expression(Exprssion::MemberExpression)` is called with `foo_exports.bar`, the span is inner StaticMemberExpression.
  fn visit_member_expression(&mut self, expr: &mut ast::MemberExpression<'ast>) {
    if let Some(new_expr) = self.try_rewrite_member_expr(expr) {
      match new_expr {
        match_member_expression!(Expression) => {
          *expr = new_expr.into_member_expression();
        }
        _ => {
          unreachable!("Always rewrite to MemberExpression for nested MemberExpression")
        }
      }
    } else {
      walk_mut::walk_member_expression(self, expr);
    }
  }

  fn visit_object_property(&mut self, prop: &mut ast::ObjectProperty<'ast>) {
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

    walk_mut::walk_object_property(self, prop);
  }

  fn visit_object_pattern(&mut self, pat: &mut ast::ObjectPattern<'ast>) {
    self.rewrite_object_pat_shorthand(pat);

    walk_mut::walk_object_pattern(self, pat);
  }

  fn visit_import_expression(&mut self, expr: &mut ast::ImportExpression<'ast>) {
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

    walk_mut::walk_import_expression(self, expr);
  }

  fn visit_assignment_target_property(
    &mut self,
    property: &mut ast::AssignmentTargetProperty<'ast>,
  ) {
    if let ast::AssignmentTargetProperty::AssignmentTargetPropertyIdentifier(prop) = property {
      if let Some(target) =
        self.generate_finalized_simple_assignment_target_for_reference(&prop.binding)
      {
        *property = ast::AssignmentTargetProperty::AssignmentTargetPropertyProperty(
          ast::AssignmentTargetPropertyProperty {
            name: ast::PropertyKey::StaticIdentifier(
              self.snippet.id_name(&prop.binding.name, prop.span).into_in(self.alloc),
            ),
            binding: if let Some(init) = prop.init.take() {
              ast::AssignmentTargetMaybeDefault::AssignmentTargetWithDefault(
                ast::AssignmentTargetWithDefault {
                  binding: ast::AssignmentTarget::from(target),
                  init,
                  span: Span::default(),
                }
                .into_in(self.alloc),
              )
            } else {
              ast::AssignmentTargetMaybeDefault::from(target)
            },
            span: Span::default(),
          }
          .into_in(self.alloc),
        );
      } else {
        prop.binding.reference_id.get_mut().take();
      }
    }

    walk_mut::walk_assignment_target_property(self, property);
  }

  fn visit_simple_assignment_target(&mut self, target: &mut SimpleAssignmentTarget<'ast>) {
    self.rewrite_simple_assignment_target(target);

    walk_mut::walk_simple_assignment_target(self, target);
  }

  fn visit_declaration(&mut self, it: &mut ast::Declaration<'ast>) {
    match it {
      ast::Declaration::VariableDeclaration(decl) => {
        match decl.declarations.as_mut_slice() {
          [decl] => {
            if let (BindingPatternKind::BindingIdentifier(id), Some(init)) =
              (&decl.id.kind, decl.init.as_mut())
            {
              match init {
                ast::Expression::ClassExpression(class_expression) => {
                  if let Some(element) = self.keep_name_helper_for_class(Some(
                    class_expression.id.as_ref().unwrap_or_else(|| id),
                  )) {
                    class_expression.body.body.insert(0, element);
                  }
                }
                ast::Expression::FunctionExpression(fn_expression) => {
                  // The `var fn = function foo() {}` shoulde be generate `__name(fn, 'foo')` to keep the name
                  self.process_fn(Some(id), Some(fn_expression.id.as_ref().unwrap_or_else(|| id)));
                }
                _ => {}
              }
            }
          }
          _ => {}
        }
      }
      ast::Declaration::FunctionDeclaration(decl) => {
        self.process_fn(decl.id.as_ref(), decl.id.as_ref());
      }
      ast::Declaration::ClassDeclaration(decl) => {
        // need to insert `keep_names` helper, because `get_transformed_class_decl`
        // will remove id in `class.id`
        if let Some(element) = self.keep_name_helper_for_class(decl.id.as_ref()) {
          decl.body.body.insert(0, element);
        }
        if let Some(decl) = self.get_transformed_class_decl(decl) {
          *it = decl;
        }
        // deconflict class name
      }
      ast::Declaration::TSTypeAliasDeclaration(_)
      | ast::Declaration::TSInterfaceDeclaration(_)
      | ast::Declaration::TSEnumDeclaration(_)
      | ast::Declaration::TSModuleDeclaration(_)
      | ast::Declaration::TSImportEqualsDeclaration(_) => unreachable!(),
    }
    walk_mut::walk_declaration(self, it);
  }
}

impl<'ast> ScopeHoistingFinalizer<'_, 'ast> {
  /// rewrite toplevel `class ClassName {}` to `var ClassName = class {}`
  fn get_transformed_class_decl(
    &mut self,
    class: &mut allocator::Box<'ast, ast::Class<'ast>>,
  ) -> Option<ast::Declaration<'ast>> {
    let scope_id = class.scope_id.get()?;

    if self.scope.get_parent_id(scope_id) != Some(self.scope.root_scope_id()) {
      return None;
    };

    let id = class.id.take()?;

    if let Some(symbol_id) = id.symbol_id.get() {
      if self.ctx.module.self_referenced_class_decl_symbol_ids.contains(&symbol_id) {
        // class T { static a = new T(); }
        // needs to rewrite to `var T = class T { static a = new T(); }`
        let mut id = id.clone();
        let new_name = self.canonical_name_for((self.ctx.id, symbol_id).into());
        id.name = self.snippet.atom(new_name);
        class.id = Some(id);
      }
    }
    Some(self.snippet.builder.declaration_variable(
      SPAN,
      VariableDeclarationKind::Var,
      self.snippet.builder.vec1(self.snippet.builder.variable_declarator(
        SPAN,
        VariableDeclarationKind::Var,
        self.snippet.builder.binding_pattern(
          ast::BindingPatternKind::BindingIdentifier(self.snippet.builder.alloc(id)),
          NONE,
          false,
        ),
        Some(Expression::ClassExpression(class.take_in(self.alloc))),
        false,
      )),
      false,
    ))
  }

  #[allow(clippy::too_many_lines, clippy::collapsible_else_if)]
  fn try_rewrite_global_require_call(
    &mut self,
    call_expr: &mut ast::CallExpression<'ast>,
  ) -> Option<Expression<'ast>> {
    if call_expr.is_global_require_call(
      self.scope,
      self.ctx.symbol_db.this_method_should_be_removed_get_symbol_table(self.ctx.id),
    ) && !call_expr.span.is_unspanned()
    {
      //  `require` calls that can't be recognized by rolldown are ignored in scanning, so they were not stored in `NomralModule#imports`.
      //  we just keep these `require` calls as it is
      if let Some(rec_id) = self.ctx.module.imports.get(&call_expr.span).copied() {
        let rec = &self.ctx.module.import_records[rec_id];
        // use `__require` instead of `require`
        if rec.meta.contains(ImportRecordMeta::CALL_RUNTIME_REQUIRE) {
          *call_expr.callee.get_inner_expression_mut() =
            self.finalized_expr_for_symbol_ref(self.canonical_ref_for_runtime("__require"), false);
        }
        let rewrite_ast = match &self.ctx.modules[rec.resolved_module] {
          Module::Normal(importee) => {
            match importee.module_type {
              ModuleType::Json => {
                // Nodejs treats json files as an esm module with a default export and rolldown follows this behavior.
                // And to make sure the runtime behavior is correct, we need to rewrite `require('xxx.json')` to `require('xxx.json').default` to align with the runtime behavior of nodejs.

                // Rewrite `require(...)` to `require_xxx(...)` or `(init_xxx(), __toCommonJS(xxx_exports).default)`
                let importee_linking_info = &self.ctx.linking_infos[importee.idx];
                let wrap_ref_name =
                  self.canonical_name_for(importee_linking_info.wrapper_ref.unwrap());
                if matches!(importee.exports_kind, ExportsKind::CommonJs) {
                  Some(self.snippet.call_expr_expr(wrap_ref_name))
                } else {
                  let ns_name = self.canonical_name_for(importee.namespace_object_ref);
                  let to_commonjs_ref_name = self.canonical_name_for_runtime("__toCommonJS");
                  Some(
                    self.snippet.seq2_in_paren_expr(
                      self.snippet.call_expr_expr(wrap_ref_name),
                      ast::Expression::StaticMemberExpression(
                        ast::StaticMemberExpression {
                          object: self
                            .snippet
                            .call_expr_with_arg_expr(to_commonjs_ref_name, ns_name),
                          property: self.snippet.id_name("default", SPAN),
                          ..TakeIn::dummy(self.alloc)
                        }
                        .into_in(self.alloc),
                      ),
                    ),
                  )
                }
              }
              _ => {
                // Rewrite `require(...)` to `require_xxx(...)` or `(init_xxx(), __toCommonJS(xxx_exports))`
                let importee_linking_info = &self.ctx.linking_infos[importee.idx];

                // `init_xxx`
                let wrap_ref_expr = self
                  .finalized_expr_for_symbol_ref(importee_linking_info.wrapper_ref.unwrap(), false);
                if matches!(importee.exports_kind, ExportsKind::CommonJs) {
                  // `init_xxx()`
                  Some(ast::Expression::CallExpression(self.snippet.builder.alloc_call_expression(
                    SPAN,
                    wrap_ref_expr,
                    NONE,
                    self.snippet.builder.vec(),
                    false,
                  )))
                } else {
                  if rec.meta.contains(ImportRecordMeta::IS_REQUIRE_UNUSED) {
                    // `init_xxx()`
                    Some(ast::Expression::CallExpression(
                      self.snippet.builder.alloc_call_expression(
                        SPAN,
                        wrap_ref_expr,
                        NONE,
                        self.snippet.builder.vec(),
                        false,
                      ),
                    ))
                  } else {
                    // `xxx_exports`
                    let namespace_object_ref_expr =
                      self.finalized_expr_for_symbol_ref(importee.namespace_object_ref, false);
                    let to_commonjs_ref = self.canonical_ref_for_runtime("__toCommonJS");
                    // `__toCommonJS`
                    let to_commonjs_expr =
                      self.finalized_expr_for_symbol_ref(to_commonjs_ref, false);

                    // `init_xxx()`
                    let wrap_ref_call_expr =
                      ast::Expression::CallExpression(self.snippet.builder.alloc_call_expression(
                        SPAN,
                        wrap_ref_expr,
                        NONE,
                        self.snippet.builder.vec(),
                        false,
                      ));

                    // `__toCommonJS(xxx_exports)`
                    let to_commonjs_call_expr =
                      ast::Expression::CallExpression(self.snippet.builder.alloc_call_expression(
                        SPAN,
                        to_commonjs_expr,
                        NONE,
                        self.snippet.builder.vec1(ast::Argument::from(namespace_object_ref_expr)),
                        false,
                      ));

                    // `(init_xxx(), __toCommonJS(xxx_exports))`
                    Some(self.snippet.seq2_in_paren_expr(wrap_ref_call_expr, to_commonjs_call_expr))
                  }
                }
              }
            }
          }
          Module::External(importee) => {
            let request_path =
              call_expr.arguments.get_mut(0).expect("require should have an argument");

            // Rewrite `require('xxx')` to `require('fs')`, if there is an alias that maps 'xxx' to 'fs'
            *request_path = ast::Argument::StringLiteral(
              self.snippet.alloc_string_literal(&importee.name, request_path.span()),
            );
            None
          }
        };
        return rewrite_ast;
      }
    }
    None
  }

  fn try_rewrite_inline_dynamic_import_expr(
    &mut self,
    import_expr: &mut ImportExpression<'ast>,
  ) -> Option<Expression<'ast>> {
    if self.ctx.options.inline_dynamic_imports {
      let rec_id = self.ctx.module.imports.get(&import_expr.span)?;
      let rec = &self.ctx.module.import_records[*rec_id];
      let importee_id = rec.resolved_module;
      match &self.ctx.modules[importee_id] {
        Module::Normal(importee) => {
          let importee_linking_info = &self.ctx.linking_infos[importee_id];
          let new_expr = match importee_linking_info.wrap_kind {
            WrapKind::Esm => {
              // Rewrite `import('./foo.mjs')` to `(init_foo(), foo_exports)`
              let importee_linking_info = &self.ctx.linking_infos[importee_id];

              // `init_foo`
              let importee_wrapper_ref_name =
                self.canonical_name_for(importee_linking_info.wrapper_ref.unwrap());

              // `foo_exports`
              let importee_namespace_name = self.canonical_name_for(importee.namespace_object_ref);

              // `(init_foo(), foo_exports)`
              Some(self.snippet.promise_resolve_then_call_expr(
                import_expr.span,
                self.snippet.builder.vec1(self.snippet.return_stmt(
                  self.snippet.seq2_in_paren_expr(
                    self.snippet.call_expr_expr(importee_wrapper_ref_name),
                    self.snippet.id_ref_expr(importee_namespace_name, SPAN),
                  ),
                )),
              ))
            }
            WrapKind::Cjs => {
              //  `__toESM(require_foo())`
              let to_esm_fn_name = self.canonical_name_for_runtime("__toESM");
              let importee_wrapper_ref_name =
                self.canonical_name_for(importee_linking_info.wrapper_ref.unwrap());

              Some(self.snippet.promise_resolve_then_call_expr(
                import_expr.span,
                self.snippet.builder.vec1(self.snippet.return_stmt(
                  self.snippet.to_esm_call_with_interop(
                    to_esm_fn_name,
                    self.snippet.call_expr_expr(importee_wrapper_ref_name),
                    importee.interop(),
                  ),
                )),
              ))
            }
            WrapKind::None => {
              // The nature of `import()` is to load the module dynamically/lazily, so imported modules would
              // must be wrapped, so we could make sure the module is executed lazily.
              if cfg!(debug_assertions) {
                unreachable!()
              }
              None
            }
          };
          return new_expr;
        }
        Module::External(_) => {
          // iife format doesn't support external module
        }
      }
    }
    if matches!(self.ctx.options.format, OutputFormat::Cjs) {
      // Convert `import('./foo.mjs')` to `Promise.resolve().then(function() { return require('foo.mjs') })`
      let rec_id = self.ctx.module.imports.get(&import_expr.span)?;
      let rec = &self.ctx.module.import_records[*rec_id];
      let importee_id = rec.resolved_module;
      match &self.ctx.modules[importee_id] {
        Module::Normal(_importee) => {
          let importer_chunk_id = self.ctx.chunk_graph.module_to_chunk[self.ctx.module.idx]
            .expect("Normal module should belong to a chunk");
          let importer_chunk = &self.ctx.chunk_graph.chunk_table[importer_chunk_id];
          let importee_chunk_id = self.ctx.chunk_graph.entry_module_to_entry_chunk[&importee_id];
          let importee_chunk = &self.ctx.chunk_graph.chunk_table[importee_chunk_id];
          let import_path = importer_chunk.import_path_for(importee_chunk);
          let new_expr = self.snippet.promise_resolve_then_call_expr(
            import_expr.span,
            self.snippet.builder.vec1(ast::Statement::ReturnStatement(
              self.snippet.builder.alloc_return_statement(
                SPAN,
                Some(ast::Expression::CallExpression(self.snippet.builder.alloc_call_expression(
                  SPAN,
                  self.snippet.builder.expression_identifier_reference(SPAN, "require"),
                  NONE,
                  self.snippet.builder.vec1(ast::Argument::StringLiteral(
                    self.snippet.alloc_string_literal(&import_path, import_expr.span),
                  )),
                  false,
                ))),
              ),
            )),
          );
          return Some(new_expr);
        }
        Module::External(_) => {
          // For `import('external')`, we just keep it as it is to preserve user's intention
        }
      }
    }
    None
  }

  #[allow(clippy::too_many_lines)]
  fn remove_unused_top_level_stmt(&mut self, program: &mut ast::Program<'ast>) {
    let old_body = self.alloc.take(&mut program.body);

    // the first statement info is the namespace variable declaration
    // skip first statement info to make sure `program.body` has same index as `stmt_infos`
    old_body.into_iter().enumerate().zip(self.ctx.module.stmt_infos.iter().skip(1)).for_each(
      |((_top_stmt_idx, mut top_stmt), stmt_info)| {
        debug_assert!(matches!(stmt_info.stmt_idx, Some(_top_stmt_idx)));
        if !stmt_info.is_included {
          return;
        }

        if let Some(import_decl) = top_stmt.as_import_declaration() {
          let rec_id = self.ctx.module.imports[&import_decl.span];
          if self.transform_or_remove_import_export_stmt(&mut top_stmt, rec_id) {
            return;
          }
        } else if let Some(export_all_decl) = top_stmt.as_export_all_declaration() {
          let rec_id = self.ctx.module.imports[&export_all_decl.span];
          // "export * as ns from 'path'"
          if let Some(_alias) = &export_all_decl.exported {
            if self.transform_or_remove_import_export_stmt(&mut top_stmt, rec_id) {
              return;
            }
          } else {
            // "export * from 'path'"
            let rec = &self.ctx.module.import_records[rec_id];
            match &self.ctx.modules[rec.resolved_module] {
              Module::Normal(importee) => {
                let importee_linking_info = &self.ctx.linking_infos[importee.idx];
                if matches!(importee_linking_info.wrap_kind, WrapKind::Esm) {
                  let wrapper_ref_name =
                    self.canonical_name_for(importee_linking_info.wrapper_ref.unwrap());
                  program.body.push(self.snippet.call_expr_stmt(wrapper_ref_name));
                }

                match importee.exports_kind {
                  ExportsKind::Esm => {
                    if importee_linking_info.has_dynamic_exports {
                      let re_export_fn_name = self.canonical_name_for_runtime("__reExport");
                      let importer_namespace_name =
                        self.canonical_name_for(self.ctx.module.namespace_object_ref);
                      // __reExport(exports, otherExports)
                      let importee_namespace_name =
                        self.canonical_name_for(importee.namespace_object_ref);
                      program.body.push(
                        self
                          .snippet
                          .call_expr_with_2arg_expr(
                            re_export_fn_name,
                            importer_namespace_name,
                            importee_namespace_name,
                          )
                          .into_in(self.alloc),
                      );
                    }
                  }
                  ExportsKind::CommonJs => {
                    let re_export_fn_name = self.canonical_name_for_runtime("__reExport");
                    let importer_namespace_name =
                      self.canonical_name_for(self.ctx.module.namespace_object_ref);
                    // __reExport(exports, __toESM(require_xxxx()))
                    let to_esm_fn_name = self.canonical_name_for_runtime("__toESM");
                    let importee_wrapper_ref_name =
                      self.canonical_name_for(importee_linking_info.wrapper_ref.unwrap());
                    program.body.push(
                      self
                        .snippet
                        .alloc_call_expr_with_2arg_expr_expr(
                          re_export_fn_name,
                          self.snippet.id_ref_expr(importer_namespace_name, SPAN),
                          self.snippet.to_esm_call_with_interop(
                            to_esm_fn_name,
                            self.snippet.call_expr_expr(importee_wrapper_ref_name),
                            importee.interop(),
                          ),
                        )
                        .into_in(self.alloc),
                    );
                  }
                  ExportsKind::None => {}
                }
              }
              Module::External(_importee) => {
                match self.ctx.options.format {
                  rolldown_common::OutputFormat::Esm
                  | rolldown_common::OutputFormat::Iife
                  | rolldown_common::OutputFormat::Umd
                  | rolldown_common::OutputFormat::Cjs => {
                    // Just remove the statement
                    return;
                  }
                  rolldown_common::OutputFormat::App => {
                    unreachable!()
                  }
                }
              }
            }

            return;
          }
        } else if let Some(default_decl) = top_stmt.as_export_default_declaration_mut() {
          use ast::ExportDefaultDeclarationKind;
          match &mut default_decl.declaration {
            decl @ ast::match_expression!(ExportDefaultDeclarationKind) => {
              let expr = decl.to_expression_mut();
              // "export default foo;" => "var default = foo;"
              let canonical_name_for_default_export_ref =
                self.canonical_name_for(self.ctx.module.default_export_ref);
              top_stmt = self
                .snippet
                .var_decl_stmt(canonical_name_for_default_export_ref, expr.take_in(self.alloc));
            }
            ast::ExportDefaultDeclarationKind::FunctionDeclaration(func) => {
              // "export default function() {}" => "function default() {}"
              // "export default function foo() {}" => "function foo() {}"
              if func.id.is_none() {
                let canonical_name_for_default_export_ref =
                  self.canonical_name_for(self.ctx.module.default_export_ref);
                func.id = Some(self.snippet.id(canonical_name_for_default_export_ref, SPAN));
              }
              top_stmt = ast::Statement::FunctionDeclaration(func.take_in(self.alloc));
            }
            ast::ExportDefaultDeclarationKind::ClassDeclaration(class) => {
              // "export default class {}" => "class default {}"
              // "export default class Foo {}" => "class Foo {}"
              if class.id.is_none() {
                let canonical_name_for_default_export_ref =
                  self.canonical_name_for(self.ctx.module.default_export_ref);
                class.id = Some(self.snippet.id(canonical_name_for_default_export_ref, SPAN));
              }
              top_stmt = ast::Statement::ClassDeclaration(class.take_in(self.alloc));
            }
            _ => {}
          }
        } else if let Some(named_decl) = top_stmt.as_export_named_declaration_mut() {
          if named_decl.source.is_none() {
            if let Some(decl) = &mut named_decl.declaration {
              // `export var foo = 1` => `var foo = 1`
              // `export function foo() {}` => `function foo() {}`
              // `export class Foo {}` => `class Foo {}`
              top_stmt = ast::Statement::from(decl.take_in(self.alloc));
            } else {
              // `export { foo }`
              // Remove this statement by ignoring it
              return;
            }
          } else {
            // `export { foo } from 'path'`
            let rec_id = self.ctx.module.imports[&named_decl.span];
            if self.transform_or_remove_import_export_stmt(&mut top_stmt, rec_id) {
              return;
            }
          }
        }

        program.body.push(top_stmt);
      },
    );
  }

  fn process_fn(
    &mut self,
    symbol_binding_id: Option<&BindingIdentifier<'ast>>,
    name_binding_id: Option<&BindingIdentifier<'ast>>,
  ) -> Option<()> {
    if !self.ctx.options.keep_names {
      return None;
    }
    let (_, original_name, _) = self.get_conflicted_info(name_binding_id.as_ref()?)?;
    let (symbol_id, _, canonical_name) = self.get_conflicted_info(symbol_binding_id.as_ref()?)?;
    let original_name: Rstr = original_name.into();
    let new_name = canonical_name.clone();
    let insert_position = self.ctx.cur_stmt_index + 1;
    self.ctx.keep_name_statement_to_insert.push((
      insert_position,
      symbol_id,
      original_name,
      new_name,
    ));
    None
  }

  fn keep_name_helper_for_class(
    &mut self,
    id: Option<&BindingIdentifier<'ast>>,
  ) -> Option<ClassElement<'ast>> {
    if !self.ctx.options.keep_names {
      return None;
    }
    let (_, original_name, _) = self.get_conflicted_info(id.as_ref()?)?;
    let original_name: Rstr = original_name.into();
    Some(self.snippet.static_block_keep_name_helper(&original_name))
  }
}
