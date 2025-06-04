use oxc::{
  allocator::{self, Dummy, IntoIn, TakeIn},
  ast::{
    ast::{self, BindingPatternKind, Expression, SimpleAssignmentTarget},
    match_member_expression,
  },
  ast_visit::{VisitMut, walk_mut},
  span::{SPAN, Span},
};
use rolldown_common::{ExportsKind, StmtInfoIdx, SymbolRef, ThisExprReplaceKind, WrapKind};
use rolldown_ecmascript_utils::{ExpressionExt, JsxExt};
use rustc_hash::FxHashMap;

use super::ScopeHoistingFinalizer;

impl<'ast> VisitMut<'ast> for ScopeHoistingFinalizer<'_, 'ast> {
  #[allow(clippy::too_many_lines)]
  fn visit_program(&mut self, program: &mut ast::Program<'ast>) {
    // Drop the hashbang since we already store them in ast_scan phase and
    // we don't want oxc to generate hashbang statement and directives in module level since we already handle
    // them in chunk level
    program.hashbang.take();
    program.directives.clear();
    // init namespace_alias_symbol_id
    self.namespace_alias_symbol_id_to_resolved_module = self
      .ctx
      .module
      .ecma_view
      .named_imports
      .iter()
      .filter_map(|(symbol_ref, v)| {
        let rec_id = v.record_id;
        let importee_idx = self.ctx.module.ecma_view.import_records[rec_id].resolved_module;
        // bailout if the importee is a external module
        // see rollup/test/function/samples/side-effects-only-default-exports/ as an
        // example
        // TODO: maybe we could relex the restriction if `platform: node` and the external module
        // is a node builtin module
        let module = self.ctx.modules[importee_idx].as_normal()?;
        self.ctx.symbol_db.get(*symbol_ref).namespace_alias.as_ref()?;
        module.exports_kind.is_commonjs().then_some((symbol_ref.symbol, importee_idx))
      })
      .collect::<FxHashMap<_, _>>();

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
      self.generate_declaration_of_module_namespace_object()
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

    let hmr_header = if self.ctx.runtime.id() == self.ctx.module.idx {
      vec![]
    } else {
      // FIXME(hyf0): Module register relies on runtime module, this causes a runtime error for registering runtime module.
      // Let's skip it for now.
      self.generate_hmr_header()
    };
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

          let commonjs_ref_expr = self.finalized_expr_for_symbol_ref(commonjs_ref, false, None);

          let mut stmts_inside_closure = allocator::Vec::new_in(self.alloc);
          stmts_inside_closure.extend(hmr_header);
          stmts_inside_closure.append(&mut program.body);

          program.body.push(self.snippet.commonjs_wrapper_stmt(
            wrap_ref_name,
            commonjs_ref_expr,
            stmts_inside_closure,
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
          let esm_ref_expr = self.finalized_expr_for_symbol_ref(esm_ref, false, None);
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
          program.body.extend(hmr_header);
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
                  ..ast::BindingPattern::dummy(self.alloc)
                },
                kind: ast::VariableDeclarationKind::Var,
                ..ast::VariableDeclarator::dummy(self.alloc)
              });
            });
            program.body.push(ast::Statement::VariableDeclaration(
              ast::VariableDeclaration {
                declarations: declarators,
                kind: ast::VariableDeclarationKind::Var,
                ..ast::VariableDeclaration::dummy(self.alloc)
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
            self.ctx.linking_info.is_tla_or_contains_tla_dependency,
          ));
        }
        WrapKind::None => {}
      }
    } else {
      program
        .body
        .splice(0..0, declaration_of_module_namespace_object.into_iter().chain(hmr_header));
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
      if let ast::Statement::LabeledStatement(stmt) = it {
        if self.ctx.options.drop_labels.contains(stmt.label.name.as_str()) {
          *it = self.snippet.builder.statement_empty(stmt.span);
        }
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
    for (stmt_index, original_name, new_name) in self.ctx.keep_name_statement_to_insert.iter().rev()
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
    self.rewrite_hot_accept_call_deps(expr);

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
        self.try_rewrite_import_expression(expr);
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
              *expr = self.snippet.builder.expression_identifier(SPAN, "exports");
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
    }
    self.rewrite_import_meta_hot(expr);

    walk_mut::walk_expression(self, expr);
  }

  fn visit_jsx_element_name(&mut self, it: &mut ast::JSXElementName<'ast>) {
    match it {
      ast::JSXElementName::Identifier(ident) => {
        walk_mut::walk_jsx_identifier(self, ident);
      }
      ast::JSXElementName::IdentifierReference(identifier_reference) => {
        if let Some(new_expr) =
          self.try_rewrite_identifier_reference_expr(identifier_reference, false)
        {
          match new_expr {
            Expression::Identifier(ident_ref) => {
              *it = ast::JSXElementName::IdentifierReference(ident_ref);
            }
            _ => {
              unreachable!(
                "Should always rewrite to Identifier for JsxElementName::IdentifierReference"
              )
            }
          }
        }
      }
      ast::JSXElementName::NamespacedName(jsx_namespace_name) => {
        walk_mut::walk_jsx_namespaced_name(self, jsx_namespace_name);
      }
      ast::JSXElementName::MemberExpression(jsx_member_expression) => {
        if let Some(ident) = jsx_member_expression.get_identifier() {
          if let Some(new_expr) = self.try_rewrite_identifier_reference_expr(ident, false) {
            match new_expr {
              Expression::Identifier(ident_ref) => {
                jsx_member_expression.object.rewrite_ident_reference(ident_ref);
              }
              _ => {
                unreachable!(
                  "Should always rewrite to Identifier for JsxMemberExpression::get_identifier()"
                )
              }
            }
          }
        }
      }
      ast::JSXElementName::ThisExpression(this_expression) => {
        walk_mut::walk_this_expression(self, this_expression);
      }
    }
  }

  // foo.js `export const bar = { a: 0 }`
  // main.js `import * as foo_exports from './foo.js';\n foo_exports.bar.a = 1;`
  // The `foo_exports.bar.a` ast is `StaticMemberExpression(StaticMemberExpression)`, The outer StaticMemberExpression span is `foo_exports.bar.a`, the `visit_expression(Expression::MemberExpression)` is called with `foo_exports.bar`, the span is inner StaticMemberExpression.
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
      if let Some(ref_id) = self.try_get_valid_namespace_alias_ref_id_from_member_expr(expr) {
        self.interested_namespace_alias_ref_id.insert(ref_id);
      }
      walk_mut::walk_member_expression(self, expr);
    }
  }

  fn visit_object_property(&mut self, prop: &mut ast::ObjectProperty<'ast>) {
    // Ensure `{ a }` would be rewritten to `{ a: a$1 }` instead of `{ a$1 }`
    if prop.shorthand {
      if let ast::Expression::Identifier(id_ref) = &mut prop.value {
        match self.generate_finalized_expr_for_reference(id_ref, false) {
          Some(expr) => {
            prop.value = expr;
            prop.shorthand = false;
          }
          None => {
            id_ref.reference_id.get_mut().take();
          }
        }
      }
    }

    walk_mut::walk_object_property(self, prop);
  }

  fn visit_object_pattern(&mut self, pat: &mut ast::ObjectPattern<'ast>) {
    self.rewrite_object_pat_shorthand(pat);

    walk_mut::walk_object_pattern(self, pat);
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
            computed: false,
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
                  // The `var fn = function foo() {}` should generate `__name(fn, 'foo')` to keep the name
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
