use itertools::Itertools;
use oxc::ast::AstType;
use oxc::ast::ast::{AssignmentTarget, JSXMemberExpression};
use oxc::{
  allocator::{self, IntoIn, ReplaceWith, TakeIn},
  ast::{
    ast::{self, BindingPattern, Expression, IdentifierName, SimpleAssignmentTarget, Statement},
    builder::NONE,
    match_member_expression,
  },
  ast_visit::{VisitMut, walk_mut},
  span::{SPAN, Span},
};
use oxc_str::CompactStr;
use rolldown_common::{ConcatenateWrappedModuleKind, SymbolRef, ThisExprReplaceKind};
use rolldown_ecmascript::ToSourceString;
use rolldown_ecmascript_utils::{
  EsmWrapperBodyKind, EsmWrapperCallKind, EsmWrapperDeclKind, EsmWrapperStmtOptions, ExpressionExt,
  ExpressionFactoryExt as _, IdentifierNameFactoryExt as _, JsxExt, JsxMemberExpressionObjectExt,
  StatementFactoryExt as _,
};
use rolldown_error::EmptyImportMetaKind;

use crate::module_finalizers::{KeepNameId, ModuleWrapperMode, TraverseState};

use super::ScopeHoistingFinalizer;

impl<'ast> VisitMut<'ast> for ScopeHoistingFinalizer<'_, 'ast> {
  fn enter_scope(
    &mut self,
    flags: oxc::semantic::ScopeFlags,
    _scope_id: &std::cell::Cell<Option<oxc::semantic::ScopeId>>,
  ) {
    self.scope_stack.push(flags);
    self.state.set(
      TraverseState::TopLevel,
      self.scope_stack.iter().rev().all(|flag| flag.is_block() || flag.is_top()),
    );
    self
      .state
      .set(TraverseState::IsRootLevel, self.scope_stack.iter().rev().all(|flag| flag.is_top()));
  }

  fn leave_scope(&mut self) {
    self.scope_stack.pop();
    self.state.set(
      TraverseState::TopLevel,
      self.scope_stack.iter().rev().all(|flag| flag.is_block() || flag.is_top()),
    );
    self
      .state
      .set(TraverseState::IsRootLevel, self.scope_stack.iter().rev().all(|flag| flag.is_top()));
  }

  fn visit_if_statement(&mut self, it: &mut ast::IfStatement<'ast>) {
    let kind = AstType::IfStatement;
    self.enter_node(kind);
    self.visit_span(&mut it.span);
    let pre = self.state;
    self.state.insert(TraverseState::SmartInlineConst);
    self.visit_expression(&mut it.test);
    self.state = pre;
    self.visit_statement(&mut it.consequent);
    if let Some(alternate) = &mut it.alternate {
      self.visit_statement(alternate);
    }
    self.leave_node(kind);
  }

  fn visit_conditional_expression(&mut self, it: &mut ast::ConditionalExpression<'ast>) {
    let kind = AstType::ConditionalExpression;
    self.enter_node(kind);
    self.visit_span(&mut it.span);
    let pre = self.state;
    self.state.insert(TraverseState::SmartInlineConst);
    self.visit_expression(&mut it.test);
    self.state = pre;
    self.visit_expression(&mut it.consequent);
    self.visit_expression(&mut it.alternate);
    self.leave_node(kind);
  }

  fn visit_logical_expression(&mut self, it: &mut ast::LogicalExpression<'ast>) {
    let pre = self.state;
    self.state.insert(TraverseState::SmartInlineConst);
    walk_mut::walk_logical_expression(self, it);
    self.state = pre;
  }

  fn visit_program(&mut self, program: &mut ast::Program<'ast>) {
    // Drop the hashbang since we already store them in ast_scan phase and
    // we don't want oxc to generate hashbang statement and directives in module level since we already handle
    // them in chunk level
    program.hashbang.take();
    program.directives.clear();
    // init namespace_alias_symbol_id

    let last_import_stmt_idx = self.remove_unused_top_level_stmt(program);

    if self.ctx.options.is_dev_mode_enabled() {
      let hmr_header = if self.ctx.runtime.id() == self.ctx.module.idx {
        vec![]
      } else {
        // FIXME(hyf0): Module register relies on runtime module, this causes a runtime error for registering runtime module.
        // Let's skip it for now.
        self.generate_hmr_header()
      };
      program.body.splice(last_import_stmt_idx..last_import_stmt_idx, hmr_header);
    }

    let wrapper_mode = self.ctx.wrapper_mode();
    self.needs_hosted_top_level_binding = matches!(
      wrapper_mode,
      ModuleWrapperMode::InteropEsm(_) | ModuleWrapperMode::ExecutionOrder(_)
    );

    // the order should be
    // 1. module namespace object declaration
    // 2. shimmed_exports
    // 3. hoisted_names
    // 4. wrapped module declaration
    let declaration_of_module_namespace_object =
      self.generate_declaration_of_module_namespace_object();

    let mut shimmed_exports =
      self.ctx.linking_info.shimmed_missing_exports.iter().collect::<Vec<_>>();
    shimmed_exports.sort_unstable_by_key(|(name, _)| name.as_str());
    shimmed_exports.into_iter().for_each(|(_name, symbol_ref)| {
      debug_assert!(!self.ctx.stmt_infos.declared_stmts_by_symbol(symbol_ref).is_empty());
      let is_included: bool = self
        .ctx
        .stmt_infos
        .declared_stmts_by_symbol(symbol_ref)
        .iter()
        .any(|id| self.ctx.linking_info.stmt_info_included.has_bit(*id));
      if is_included {
        let canonical_name = self.canonical_name_for(*symbol_ref);
        program.body.push(Statement::new_var_decl(
          canonical_name,
          ast::Expression::new_void_0(SPAN, &self.ast_builder),
          &self.ast_builder,
        ));
      }
    });

    walk_mut::walk_program(self, program);

    // Insert keep_name statements for top-level declarations
    self.insert_keep_name_statements(&mut program.body);
    self.keep_name_statement_to_insert.clear();

    match wrapper_mode {
      ModuleWrapperMode::InteropCjs(wrapper_ref) => {
        let wrap_ref_name = self.canonical_name_for(wrapper_ref);
        let commonjs_ref = if self.ctx.options.profiler_names {
          self.canonical_ref_for_runtime("__commonJS")
        } else {
          self.canonical_ref_for_runtime("__commonJSMin")
        };

        let (commonjs_ref_expr, _) = self.finalized_expr_for_symbol_ref(commonjs_ref, false, false);

        let mut stmts_inside_closure = allocator::Vec::new_in(&self.alloc);
        stmts_inside_closure.append(&mut program.body);

        program.body.push(Statement::new_commonjs_wrapper_stmt(
          wrap_ref_name,
          commonjs_ref_expr,
          stmts_inside_closure,
          self.ctx.module.ast_usage,
          self.ctx.options.profiler_names,
          &self.ctx.module.stable_id,
          self.ctx.linking_info.is_tla_or_contains_tla_dependency,
          &self.ast_builder,
        ));
      }
      ModuleWrapperMode::InteropEsm(target) | ModuleWrapperMode::ExecutionOrder(target) => {
        let is_concatenated_wrapped_module = !matches!(
          self.ctx.linking_info.concatenated_wrapped_module_kind,
          ConcatenateWrappedModuleKind::None
        );
        let old_body = program.body.take_in(&self.alloc);

        let mut fn_stmts = allocator::Vec::new_in(&self.alloc);
        let mut stmts_inside_closure = allocator::Vec::new_in(&self.alloc);

        // Hoist all top-level "var" and "function" declarations out of the closure
        old_body.into_iter().for_each(|mut stmt| match &mut stmt {
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

        // Safety net for the `init_is_noop` predictive classifier (see
        // `generate_stage::compute_wrapped_esm_init_metadata`): if a module was flagged as
        // having an empty `__esm` closure, the statements that actually land inside the closure
        // must be empty.
        // Otherwise we'd have marked a side-effecting `init_*()` as `@__PURE__` and DCE could
        // wrongly drop it. Turns any misclassification into a loud failure across the fixtures.
        debug_assert!(
          !self.ctx.final_esm_init_metadata.init_is_noop(self.ctx.idx)
            || stmts_inside_closure.is_empty(),
          "init_is_noop set but the __esm closure is non-empty for {}",
          self.ctx.module.stable_id
        );

        if is_concatenated_wrapped_module {
          self.rendered_concatenated_wrapped_module_parts.hoisted_functions_or_module_ns_decl =
            declaration_of_module_namespace_object
              .iter()
              .chain(fn_stmts.iter())
              .map(rolldown_ecmascript::ToSourceString::to_source_string)
              .collect_vec();
          self.rendered_concatenated_wrapped_module_parts.hoisted_vars = self
            .top_level_var_bindings
            .iter()
            .map(|var_name| CompactStr::new(var_name))
            .collect_vec();
        } else {
          program.body.extend(declaration_of_module_namespace_object);
          program.body.extend(fn_stmts);
        }

        if !is_concatenated_wrapped_module && !self.top_level_var_bindings.is_empty() {
          let ast_builder = &self.ast_builder;
          let decorations = self.top_level_var_bindings.iter().map(|var_name| {
            ast::VariableDeclarator::new(
              SPAN,
              ast::VariableDeclarationKind::Var,
              ast::BindingPattern::new_binding_identifier(SPAN, *var_name, ast_builder),
              NONE,
              None,
              false,
              ast_builder,
            )
          });
          program.body.push(Statement::new_variable_declaration(
            SPAN,
            ast::VariableDeclarationKind::Var,
            oxc::allocator::Vec::from_iter_in(decorations, ast_builder),
            false,
            ast_builder,
          ));
        }

        // The wrapping would happen during the chunk codegen phase
        if matches!(
          self.ctx.linking_info.concatenated_wrapped_module_kind,
          ConcatenateWrappedModuleKind::Inner
        ) {
          program.body.extend(stmts_inside_closure);
          return;
        }

        let esm_ref = if self.ctx.options.profiler_names {
          self.canonical_ref_for_runtime("__esm")
        } else {
          self.canonical_ref_for_runtime("__esmMin")
        };
        let (esm_ref_expr, _) = self.finalized_expr_for_symbol_ref(esm_ref, false, false);
        let wrap_ref_name = self.canonical_name_for(target.wrapper_ref);

        if matches!(
          self.ctx.linking_info.concatenated_wrapped_module_kind,
          ConcatenateWrappedModuleKind::Root
        ) {
          self.rendered_concatenated_wrapped_module_parts.rendered_esm_runtime_expr = Some(
            ast::ExpressionStatement::new(SPAN, esm_ref_expr, &self.ast_builder).to_source_string(),
          );
          self.rendered_concatenated_wrapped_module_parts.wrap_ref_name =
            Some(CompactStr::new(wrap_ref_name));
          program.body.extend(stmts_inside_closure);
          return;
        }

        program.body.push(Statement::new_esm_wrapper_stmt(
          EsmWrapperStmtOptions {
            binding_name: wrap_ref_name,
            esm_fn_expr: esm_ref_expr,
            statements: stmts_inside_closure,
            profiler_name: self
              .ctx
              .options
              .profiler_names
              .then_some(self.ctx.module.stable_id.as_str()),
            call_kind: if self.ctx.options.optimization.is_pife_for_module_wrappers_enabled() {
              EsmWrapperCallKind::Pife
            } else {
              EsmWrapperCallKind::Plain
            },
            body_kind: if self.ctx.linking_info.is_tla_or_contains_tla_dependency {
              EsmWrapperBodyKind::Async
            } else {
              EsmWrapperBodyKind::Sync
            },
            decl_kind: if matches!(wrapper_mode, ModuleWrapperMode::ExecutionOrder(_)) {
              EsmWrapperDeclKind::HoistedFunction
            } else {
              EsmWrapperDeclKind::Var
            },
          },
          &self.ast_builder,
        ));
      }
      ModuleWrapperMode::None => {
        program.body.splice(0..0, declaration_of_module_namespace_object);
      }
    }

    if self.json_module_inlined_prop.is_some() {
      program.body.drain_filter(|item| matches!(item, ast::Statement::EmptyStatement(_)));
    }
  }

  fn visit_binding_identifier(&mut self, ident: &mut ast::BindingIdentifier<'ast>) {
    if let Some(symbol_id) = ident.symbol_id.get() {
      let symbol_ref: SymbolRef = (self.ctx.idx, symbol_id).into();

      let canonical_ref = self.ctx.symbol_db.canonical_ref_for(symbol_ref);
      let symbol = self.ctx.symbol_db.get(canonical_ref);
      assert!(symbol.namespace_alias.is_none());
      let canonical_name = self.canonical_name_for(symbol_ref);
      if ident.name != canonical_name {
        ident.name = oxc::ast::ast::Str::from_str_in(canonical_name, &self.ast_builder).into();
      }
      ident.symbol_id.get_mut().take();
    } else {
      // Some `BindingIdentifier`s constructed by bundler don't have `SymbolId` and we just ignore them.
    }
  }

  fn visit_statement(&mut self, it: &mut ast::Statement<'ast>) {
    _ = self.try_inline_json_module_prop(it);

    walk_mut::walk_statement(self, it);

    // transform top level `var a = 1, b = 1;` to `a = 1, b = 1`
    // for `__esm(() => {})` wrapping VariableDeclaration hoist
    if self.state.contains(TraverseState::TopLevel)
      && self.needs_hosted_top_level_binding
      && let ast::Statement::VariableDeclaration(decl) = it
    {
      if let Some((expr, bindings)) =
        self.var_declaration_to_expr_seq_and_bindings(decl, self.state)
      {
        self.top_level_var_bindings.extend(bindings);
        *it = ast::Statement::new_expression_statement(SPAN, expr, &self.ast_builder);
      }
    }
  }

  fn visit_statements(&mut self, it: &mut allocator::Vec<'ast, ast::Statement<'ast>>) {
    let previous_stmt_index = self.cur_stmt_index;
    let previous_keep_name_statement = std::mem::take(&mut self.keep_name_statement_to_insert);
    for (i, stmt) in it.iter_mut().enumerate() {
      self.cur_stmt_index = i;
      self.visit_statement(stmt);
    }

    self.insert_keep_name_statements(it);
    self.cur_stmt_index = previous_stmt_index;
    self.keep_name_statement_to_insert = previous_keep_name_statement;
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

  fn visit_expression(&mut self, expr: &mut ast::Expression<'ast>) {
    // Handle keep_names for named class/function expressions in any expression context
    // (return statements, function args, array elements, etc.)
    if self.ctx.options.keep_names && self.ctx.runtime.id() != self.ctx.idx {
      match expr {
        ast::Expression::ClassExpression(class_expression) => {
          if let Some(id) = class_expression.id.as_ref() {
            if let Some(element) = self.keep_name_helper_for_class(
              id.symbol_id.get().map(KeepNameId::SymbolId),
              &class_expression.body,
            ) {
              class_expression.body.body.insert(0, element);
            }
          }
        }
        ast::Expression::FunctionExpression(fn_expression) => {
          if let Some(id) = fn_expression.id.as_mut() {
            if let Some(symbol_id) = id.symbol_id.get() {
              let keep_name_id = KeepNameId::SymbolId(symbol_id);
              if let Some((_insert_position, original_name, _)) =
                self.process_fn(Some(keep_name_id), Some(keep_name_id))
              {
                // Manually rename the binding identifier before clearing symbol_id
                let symbol_ref: SymbolRef = (self.ctx.idx, symbol_id).into();
                let canonical_name = self.canonical_name_for(symbol_ref);
                if id.name != canonical_name {
                  id.name =
                    oxc::ast::ast::Str::from_str_in(canonical_name, &self.ast_builder).into();
                }
                // Clear symbol_id to prevent double processing:
                // - visit_expression won't re-wrap when walker visits inner fn
                // - visit_binding_identifier won't re-rename
                id.symbol_id.get_mut().take();

                let name_ref = self.canonical_ref_for_runtime("__name");
                let (finalized_callee, _) =
                  self.finalized_expr_for_symbol_ref(name_ref, false, false);
                expr.replace_with(|fn_expr| {
                  Expression::new_keep_name_call(
                    &original_name,
                    fn_expr,
                    finalized_callee,
                    true,
                    &self.ast_builder,
                  )
                });
              }
            }
          }
        }
        _ => {}
      }
    }

    match expr {
      ast::Expression::CallExpression(call_expr) => {
        self.rewrite_hot_accept_call_deps(call_expr);
        if let Some(new_expr) = self.try_rewrite_global_require_call(call_expr) {
          *expr = new_expr;
        } else if let Some(ident_ref) = call_expr.callee.as_identifier_mut() {
          let is_empty_function = ident_ref
            .reference_id
            .get()
            .and_then(|ref_id| self.scope.scoping().get_reference(ref_id).symbol_id())
            .map(|id| {
              let symbol_ref = self.ctx.symbol_db.canonical_ref_for((self.ctx.idx, id).into());
              symbol_ref.is_side_effect_free_function(self.ctx.symbol_db, self.ctx.modules)
                && symbol_ref.is_not_reassigned(self.ctx.symbol_db)
            })
            .unwrap_or(false);
          if is_empty_function {
            call_expr.pure = true;
          } else if let Some(new_expr) = self.try_rewrite_identifier_reference_expr(ident_ref, true)
          {
            call_expr.callee = new_expr;
          }
        }
      }
      // inline dynamic import
      ast::Expression::ImportExpression(import_expr) => {
        if let Some(new_expr) = self.try_rewrite_inline_dynamic_import_expr(import_expr) {
          *expr = new_expr;
        }
        if self.try_rewrite_import_expression(expr) {
          // If the import expression is rewritten, we don't need to walk it again.
          // Otherwise, it might cause infinite recursion in some cases.
          return;
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
        if let Some(kind) =
          self.ctx.module.ecma_view.this_expr_replace_map.get(&this_expr.node_id())
        {
          match kind {
            ThisExprReplaceKind::Exports => {
              *expr = ast::Expression::new_identifier(SPAN, "exports", &self.ast_builder);
            }
            ThisExprReplaceKind::Context if self.ctx.options.context.is_empty() => {
              *expr = ast::Expression::new_void_0(SPAN, &self.ast_builder);
            }
            ThisExprReplaceKind::Context => {
              *expr = Expression::new_id_ref_expr(
                SPAN,
                self.ctx.options.context.as_str(),
                &self.ast_builder,
              );
            }
          }
        }
      }
      ast::Expression::ImportMeta(import_meta) => {
        if !self.ctx.options.format.keep_esm_import_export_syntax() {
          self.record_surviving_import_meta(import_meta.span, EmptyImportMetaKind::Plain);
          *expr = ast::Expression::new_object_expression(
            SPAN,
            oxc::allocator::Vec::new_in(&self.ast_builder),
            &self.ast_builder,
          );
        }
      }
      ast::Expression::ChainExpression(_) => {
        // Try inline as enum access first (`E?.x` → literal). Enum bindings are
        // always defined (the IIFE produces `{}`, never null/undefined), so `?.`
        // is equivalent to `.` here.
        if self.ctx.has_enum_inlining
          && let Some(new_expr) = self.try_inline_enum_access(expr)
        {
          *expr = new_expr;
          self.rewrite_import_meta_hot(expr);
          walk_mut::walk_expression(self, expr);
          return;
        }

        let ast::Expression::ChainExpression(chain_expr) = expr else { unreachable!() };
        // import.meta.hot?.accept()
        if let ast::ChainElement::CallExpression(call_expr) = &mut chain_expr.expression {
          self.rewrite_hot_accept_call_deps(call_expr);
        }
        let chain_span = chain_expr.span;
        if let Some(new_expr) = chain_expr
          .expression
          .as_member_expression_mut()
          .and_then(|expr| self.try_rewrite_member_expr(expr))
        {
          // If the rewritten expression contains optional member accesses (?.),
          // it must remain wrapped in a ChainExpression for valid JavaScript output.
          if has_optional_member_access(&new_expr) {
            match new_expr {
              ast::Expression::StaticMemberExpression(member) => {
                *expr = ast::Expression::new_chain_expression(
                  chain_span,
                  ast::ChainElement::StaticMemberExpression(member),
                  &self.ast_builder,
                );
              }
              ast::Expression::ComputedMemberExpression(member) => {
                *expr = ast::Expression::new_chain_expression(
                  chain_span,
                  ast::ChainElement::ComputedMemberExpression(member),
                  &self.ast_builder,
                );
              }
              _ => {
                *expr = new_expr;
              }
            }
          } else {
            *expr = new_expr;
          }
        }
      }
      _ => {
        // Try to inline enum member accesses (e.g., `Direction.Up` → `0`, `ns.c.x` → `"c"`)
        if self.ctx.has_enum_inlining {
          if let Some(new_expr) = self.try_inline_enum_access(expr) {
            *expr = new_expr;
            self.rewrite_import_meta_hot(expr);
            walk_mut::walk_expression(self, expr);
            return;
          }
        }
        if let Some(new_expr) =
          expr.as_member_expression().and_then(|expr| self.try_rewrite_member_expr(expr))
        {
          *expr = new_expr;
          // After namespace rewriting (e.g., `ns.c` → `c`), the result may be
          // an enum member access that can be inlined.
          if self.ctx.has_enum_inlining {
            if let Some(inlined) = self.try_inline_enum_access(expr) {
              *expr = inlined;
            }
          }
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
            Expression::StaticMemberExpression(member_expr) => {
              *it = ast::JSXElementName::MemberExpression(oxc::allocator::Box::new_in(
                JSXMemberExpression::from_ast(member_expr.unbox(), self.alloc).unwrap(),
                &self.alloc,
              ));
            }
            Expression::ThisExpression(this_expr) => {
              *it = ast::JSXElementName::ThisExpression(this_expr);
            }
            _ => {
              unreachable!(
                "Should always rewrite to Identifier, StaticMemberExpression, or ThisExpression for JsxElementName::IdentifierReference"
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
                jsx_member_expression.object.rewrite_ident_reference(
                  ast::JSXMemberExpressionObject::IdentifierReference(ident_ref),
                );
              }
              Expression::StaticMemberExpression(member_expr) => {
                jsx_member_expression.object.rewrite_ident_reference(
                  ast::JSXMemberExpressionObject::MemberExpression(oxc::allocator::Box::new_in(
                    // TODO: Currently only support `StaticMemberExpression`, `ThisExpression` and `IdentifierReference`.
                    // In most of scenarios, it should be enough. The ultimate solution is create
                    // an extra binding for the cjs property access then *Uppercase* the binding.
                    JSXMemberExpression::from_ast(member_expr.unbox(), self.alloc).unwrap(),
                    &self.alloc,
                  )),
                );
              }
              Expression::ThisExpression(this_expr) => {
                *it = ast::JSXElementName::ThisExpression(this_expr);
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
        let binding = if let Some(init) = prop.init.take() {
          ast::AssignmentTargetMaybeDefault::new_assignment_target_with_default(
            Span::default(),
            ast::AssignmentTarget::from(target),
            init,
            &self.ast_builder,
          )
        } else {
          ast::AssignmentTargetMaybeDefault::from(target)
        };
        *property = ast::AssignmentTargetProperty::new_assignment_target_property_property(
          Span::default(),
          ast::PropertyKey::StaticIdentifier(
            IdentifierName::new_id_name(prop.span, &prop.binding.name, &self.ast_builder)
              .into_in(self.alloc),
          ),
          binding,
          false,
          &self.ast_builder,
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

  fn visit_assignment_expression(&mut self, it: &mut ast::AssignmentExpression<'ast>) {
    if let AssignmentTarget::AssignmentTargetIdentifier(id) = &mut it.left {
      self.process_keep_name_for_expression(
        id.reference_id.get().map(KeepNameId::ReferenceId),
        &mut it.right,
      );
    }
    walk_mut::walk_assignment_expression(self, it);
  }

  fn visit_for_statement_init(&mut self, it: &mut ast::ForStatementInit<'ast>) {
    walk_mut::walk_for_statement_init(self, it);
    if self.state.contains(TraverseState::TopLevel)
      && self.needs_hosted_top_level_binding
      && let ast::ForStatementInit::VariableDeclaration(decl) = it
    {
      if let Some((expr, bindings)) =
        self.var_declaration_to_expr_seq_and_bindings(decl, self.state)
      {
        self.top_level_var_bindings.extend(bindings);
        *it = ast::ForStatementInit::from(expr);
      }
    }
  }

  fn visit_declaration(&mut self, it: &mut ast::Declaration<'ast>) {
    // keep_name transformation
    match it {
      ast::Declaration::VariableDeclaration(decl) => {
        for decl in &mut decl.declarations {
          let (BindingPattern::BindingIdentifier(id), Some(init)) = (&decl.id, decl.init.as_mut())
          else {
            continue;
          };
          self.process_keep_name_for_expression(id.symbol_id.get().map(KeepNameId::SymbolId), init);
        }
      }
      ast::Declaration::FunctionDeclaration(decl) => {
        let keep_name_id =
          decl.id.as_ref().and_then(|id| id.symbol_id.get().map(KeepNameId::SymbolId));
        if let Some((insert_position, original_name, new_name)) =
          self.process_fn(keep_name_id, keep_name_id)
        {
          self.keep_name_statement_to_insert.push((insert_position, original_name, new_name));
        }
      }
      ast::Declaration::ClassDeclaration(decl) => {
        // need to insert `keep_names` helper, because `get_transformed_class_decl`
        // will remove id in `class.id`
        if let Some(element) = self.keep_name_helper_for_class(
          decl.id.as_ref().and_then(|id| id.symbol_id.get().map(KeepNameId::SymbolId)),
          &decl.body,
        ) {
          decl.body.body.insert(0, element);
        }
        it.replace_with(|old| {
          let ast::Declaration::ClassDeclaration(class_box) = old else { unreachable!() };
          match self.get_transformed_class_decl(class_box) {
            Ok(new_decl) => new_decl,
            Err(class_box) => ast::Declaration::ClassDeclaration(class_box),
          }
        });
        // Clear symbol_id on class expression's id to prevent visit_expression
        // from inserting a duplicate __name static block during walk
        // (`it` is only a `VariableDeclaration` when the class was transformed above).
        if let ast::Declaration::VariableDeclaration(var_decl) = it {
          if let Some(declarator) = var_decl.declarations.first_mut() {
            if let Some(ast::Expression::ClassExpression(class_expr)) = &mut declarator.init {
              if let Some(id) = &mut class_expr.id {
                id.symbol_id.get_mut().take();
              }
            }
          }
        }
      }
      ast::Declaration::TSTypeAliasDeclaration(_)
      | ast::Declaration::TSInterfaceDeclaration(_)
      | ast::Declaration::TSEnumDeclaration(_)
      | ast::Declaration::TSModuleDeclaration(_)
      | ast::Declaration::TSImportEqualsDeclaration(_)
      | ast::Declaration::TSGlobalDeclaration(_) => unreachable!(),
    }

    walk_mut::walk_declaration(self, it);
  }
}

/// Check if an expression tree contains any optional member accesses (`?.`).
fn has_optional_member_access(expr: &Expression) -> bool {
  let mut cur = expr;
  loop {
    match cur {
      Expression::StaticMemberExpression(e) => {
        if e.optional {
          return true;
        }
        cur = &e.object;
      }
      Expression::ComputedMemberExpression(e) => {
        if e.optional {
          return true;
        }
        cur = &e.object;
      }
      _ => return false,
    }
  }
}
