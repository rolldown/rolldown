use itertools::Itertools;
use oxc::allocator::FromIn;
use oxc::ast::AstType;
use oxc::span::{Atom, CompactStr};
use oxc::{
  allocator::{self, IntoIn, TakeIn},
  ast::{
    NONE,
    ast::{self, BindingPatternKind, Expression, SimpleAssignmentTarget, Statement},
    match_member_expression,
  },
  ast_visit::{VisitMut, walk_mut},
  semantic::ScopeFlags,
  span::{SPAN, Span},
};
use rolldown_common::{
  ConcatenateWrappedModuleKind, ExportsKind, ModuleNamespaceIncludedReason, SymbolRef,
  ThisExprReplaceKind, WrapKind,
};
use rolldown_ecmascript::ToSourceString;
use rolldown_ecmascript_utils::{ExpressionExt, JsxExt};

use crate::hmr::utils::HmrAstBuilder;
use crate::module_finalizers::TraverseState;

use super::ScopeHoistingFinalizer;

impl<'ast> VisitMut<'ast> for ScopeHoistingFinalizer<'_, 'ast> {
  fn enter_scope(
    &mut self,
    flags: oxc::semantic::ScopeFlags,
    _scope_id: &std::cell::Cell<Option<oxc::semantic::ScopeId>>,
  ) {
    self.scope_stack.push(flags);
    self.state.set(
      TraverseState::IsTopLevel,
      self.scope_stack.iter().rev().all(|flag| flag.is_block() || flag.is_top()),
    );
  }

  fn leave_scope(&mut self) {
    self.scope_stack.pop();
    self.state.set(
      TraverseState::IsTopLevel,
      self.scope_stack.iter().rev().all(|flag| flag.is_block() || flag.is_top()),
    );
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

  #[expect(clippy::too_many_lines)]
  fn visit_program(&mut self, program: &mut ast::Program<'ast>) {
    // Drop the hashbang since we already store them in ast_scan phase and
    // we don't want oxc to generate hashbang statement and directives in module level since we already handle
    // them in chunk level
    program.hashbang.take();
    program.directives.clear();
    // init namespace_alias_symbol_id
    let is_namespace_referenced = matches!(self.ctx.module.exports_kind, ExportsKind::Esm)
      && if self
        .ctx
        .linking_info
        .module_namespace_included_reason
        .contains(ModuleNamespaceIncludedReason::Unknown)
      {
        true
      } else if self
        .ctx
        .linking_info
        .module_namespace_included_reason
        .contains(ModuleNamespaceIncludedReason::ReExportExternalModule)
      {
        // If the module namespace is only used to reexport external module,
        // then we need to ensure if it is still has dynamic exports after flatten entry level
        // external module, see `find_entry_level_external_module`
        self.ctx.linking_info.has_dynamic_exports
      } else {
        false
      };

    let last_import_stmt_idx = self.remove_unused_top_level_stmt(program);

    if self.ctx.options.is_hmr_enabled() {
      let hmr_header = if self.ctx.runtime.id() == self.ctx.module.idx {
        vec![]
      } else {
        // FIXME(hyf0): Module register relies on runtime module, this causes a runtime error for registering runtime module.
        // Let's skip it for now.
        self.generate_hmr_header()
      };
      program.body.splice(last_import_stmt_idx..last_import_stmt_idx, hmr_header);
    }

    // check if we need to add wrapper
    let included_wrap_kind = self
      .ctx
      .linking_info
      .wrapper_stmt_info
      .is_some_and(|idx| self.ctx.module.stmt_infos[idx].is_included)
      .then_some(self.ctx.linking_info.wrap_kind());

    self.ctx.needs_hosted_top_level_binding = matches!(included_wrap_kind, Some(WrapKind::Esm));

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
    walk_mut::walk_program(self, program);
    self.leave_scope();

    match included_wrap_kind {
      Some(WrapKind::Cjs) => {
        let wrap_ref_name = self.canonical_name_for(self.ctx.linking_info.wrapper_ref.unwrap());
        let commonjs_ref = if self.ctx.options.profiler_names {
          self.canonical_ref_for_runtime("__commonJS")
        } else {
          self.canonical_ref_for_runtime("__commonJSMin")
        };

        let commonjs_ref_expr = self.finalized_expr_for_symbol_ref(commonjs_ref, false, false);

        let mut stmts_inside_closure = allocator::Vec::new_in(self.alloc);
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
      Some(WrapKind::Esm) => {
        let is_concatenated_wrapped_module = !matches!(
          self.ctx.linking_info.concatenated_wrapped_module_kind,
          ConcatenateWrappedModuleKind::None
        );
        let old_body = program.body.take_in(self.alloc);

        let mut fn_stmts = allocator::Vec::new_in(self.alloc);
        let mut stmts_inside_closure = allocator::Vec::new_in(self.alloc);

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

        if is_concatenated_wrapped_module {
          self.ctx.rendered_concatenated_wrapped_module_parts.hoisted_functions_or_module_ns_decl =
            declaration_of_module_namespace_object
              .iter()
              .chain(fn_stmts.iter())
              .map(rolldown_ecmascript::ToSourceString::to_source_string)
              .collect_vec();
          self.ctx.rendered_concatenated_wrapped_module_parts.hoisted_vars = self
            .top_level_var_bindings
            .iter()
            .map(|var_name| CompactStr::new(var_name))
            .collect_vec();
        } else {
          program.body.extend(declaration_of_module_namespace_object);
          program.body.extend(fn_stmts);
        }

        if !is_concatenated_wrapped_module && !self.top_level_var_bindings.is_empty() {
          let builder = self.builder();
          let decorations = self.top_level_var_bindings.iter().map(|var_name| {
            builder.variable_declarator(
              SPAN,
              ast::VariableDeclarationKind::Var,
              builder.binding_pattern(
                BindingPatternKind::BindingIdentifier(
                  builder.alloc_binding_identifier(SPAN, *var_name),
                ),
                NONE,
                false,
              ),
              None,
              false,
            )
          });
          program.body.push(Statement::VariableDeclaration(builder.alloc_variable_declaration(
            SPAN,
            ast::VariableDeclarationKind::Var,
            builder.vec_from_iter(decorations),
            false,
          )));
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
        let esm_ref_expr = self.finalized_expr_for_symbol_ref(esm_ref, false, false);
        let wrap_ref_name = self.canonical_name_for(self.ctx.linking_info.wrapper_ref.unwrap());

        if matches!(
          self.ctx.linking_info.concatenated_wrapped_module_kind,
          ConcatenateWrappedModuleKind::Root
        ) {
          self.ctx.rendered_concatenated_wrapped_module_parts.rendered_esm_runtime_expr =
            Some(self.builder().expression_statement(SPAN, esm_ref_expr).to_source_string());
          self.ctx.rendered_concatenated_wrapped_module_parts.wrap_ref_name =
            Some(wrap_ref_name.clone());
          program.body.extend(stmts_inside_closure);
          return;
        }

        program.body.push(self.snippet.esm_wrapper_stmt(
          wrap_ref_name,
          esm_ref_expr,
          stmts_inside_closure,
          self.ctx.options.profiler_names,
          self.ctx.options.optimization.is_pife_for_module_wrappers_enabled(),
          &self.ctx.module.stable_id,
          self.ctx.linking_info.is_tla_or_contains_tla_dependency,
        ));
      }
      Some(WrapKind::None) => {}
      None => {
        program.body.splice(0..0, declaration_of_module_namespace_object);
      }
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
    // transform top level `var a = 1, b = 1;` to `a = 1, b = 1`
    // for `__esm(() => {})` wrapping VariableDeclaration hoist
    if self.state.contains(TraverseState::IsTopLevel)
      && self.ctx.needs_hosted_top_level_binding
      && let ast::Statement::VariableDeclaration(decl) = it
    {
      let (expr, bindings) =
        self.var_declaration_to_expr_seq_and_bindings(decl.take_in(self.alloc));
      self.top_level_var_bindings.extend(bindings);
      *it =
        ast::Statement::ExpressionStatement(self.builder().alloc_expression_statement(SPAN, expr));
    }
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
      let name_ref = self.canonical_ref_for_runtime("__name");
      let finalized_callee = self.finalized_expr_for_symbol_ref(name_ref, false, false);
      let target =
        self.snippet.builder.expression_identifier(SPAN, self.snippet.builder.atom(new_name));
      it.insert(
        *stmt_index,
        self.snippet.builder.statement_expression(
          SPAN,
          self.snippet.keep_name_call_expr(original_name, target, finalized_callee, false),
        ),
      );
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
        if let Some(kind) = self.ctx.module.ecma_view.this_expr_replace_map.get(&this_expr.span) {
          match kind {
            ThisExprReplaceKind::Exports => {
              *expr = self.snippet.builder.expression_identifier(SPAN, "exports");
            }
            ThisExprReplaceKind::Context if self.ctx.options.context.is_empty() => {
              *expr = self.snippet.void_zero();
            }
            ThisExprReplaceKind::Context => {
              *expr = self.snippet.builder.expression_identifier(
                SPAN,
                Atom::from_in(self.ctx.options.context.as_str(), self.alloc),
              );
            }
          }
        }
      }
      ast::Expression::MetaProperty(meta) => {
        if !self.ctx.options.format.keep_esm_import_export_syntax()
          && meta.meta.name == "import"
          && meta.property.name == "meta"
        {
          *expr = self.snippet.builder.expression_object(SPAN, self.snippet.builder.vec());
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

  fn visit_for_statement_init(&mut self, it: &mut ast::ForStatementInit<'ast>) {
    walk_mut::walk_for_statement_init(self, it);
    if self.state.contains(TraverseState::IsTopLevel)
      && self.ctx.needs_hosted_top_level_binding
      && let ast::ForStatementInit::VariableDeclaration(decl) = it
    {
      let (expr, bindings) =
        self.var_declaration_to_expr_seq_and_bindings(decl.take_in(self.alloc));
      self.top_level_var_bindings.extend(bindings);
      *it = ast::ForStatementInit::from(expr);
    }
  }

  fn visit_declaration(&mut self, it: &mut ast::Declaration<'ast>) {
    // keep_name transformation
    match it {
      ast::Declaration::VariableDeclaration(decl) => {
        for decl in &mut decl.declarations {
          let (BindingPatternKind::BindingIdentifier(id), Some(init)) =
            (&decl.id.kind, decl.init.as_mut())
          else {
            continue;
          };
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
              if let Some((_insert_position, original_name, _)) =
                self.process_fn(Some(id), Some(fn_expression.id.as_ref().unwrap_or_else(|| id)))
              {
                let fn_expr = init.take_in(self.alloc);

                let name_ref = self.canonical_ref_for_runtime("__name");
                let finalized_callee = self.finalized_expr_for_symbol_ref(name_ref, false, false);
                *init =
                  self.snippet.keep_name_call_expr(&original_name, fn_expr, finalized_callee, true);
              }
            }
            _ => {}
          }
        }
      }
      ast::Declaration::FunctionDeclaration(decl) => {
        if let Some((insert_position, original_name, new_name)) =
          self.process_fn(decl.id.as_ref(), decl.id.as_ref())
        {
          self.ctx.keep_name_statement_to_insert.push((insert_position, original_name, new_name));
        }
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
