use oxc::{
  allocator::{GetAllocator, TakeIn},
  ast::ast::{self, Expression, NumberBase},
  span::SPAN,
};
use oxc_traverse::Traverse;
use rolldown_common::{EcmaModuleAstUsage, ThisExprReplaceKind};
use rolldown_ecmascript::{
  CJS_EXPORTS_REF_STR, CJS_MODULE_REF_STR, CJS_ROLLDOWN_EXPORTS_REF,
  CJS_ROLLDOWN_EXPORTS_REF_IDENT, CJS_ROLLDOWN_MODULE_REF, CJS_ROLLDOWN_MODULE_REF_IDENT,
};
use rolldown_ecmascript_utils::{ExpressionFactoryExt as _, parse_injected_expression};
use rustc_hash::FxHashSet;

use crate::hmr::{
  hmr_ast_finalizer::HmrAstFinalizer,
  utils::{HmrAstBuilder, MODULE_ID_PARAM_FOR_HMR},
};

impl<'ast> Traverse<'ast, ()> for HmrAstFinalizer<'_, 'ast> {
  fn exit_object_property(
    &mut self,
    node: &mut ast::ObjectProperty<'ast>,
    _ctx: &mut oxc_traverse::TraverseCtx<'ast, ()>,
  ) {
    if self.module.module_type == rolldown_common::ModuleType::Json
      && self.module.meta.has_lazy_export()
      && node.key.static_name().is_some_and(|key| key.as_str() == Some("__proto__"))
    {
      node.computed = true;
    }
  }

  fn exit_simple_assignment_target(
    &mut self,
    node: &mut ast::SimpleAssignmentTarget<'ast>,
    ctx: &mut oxc_traverse::TraverseCtx<'ast, ()>,
  ) {
    if let ast::SimpleAssignmentTarget::AssignmentTargetIdentifier(reference) = node
      && let Some(target) = self.imported_assignment_target(reference, ctx)
    {
      *node = target;
    }
  }

  fn exit_assignment_target_property(
    &mut self,
    node: &mut ast::AssignmentTargetProperty<'ast>,
    ctx: &mut oxc_traverse::TraverseCtx<'ast, ()>,
  ) {
    if let ast::AssignmentTargetProperty::AssignmentTargetPropertyIdentifier(property) = node
      && let Some(target) = self.imported_assignment_target(&property.binding, ctx)
    {
      let symbol = ctx.scoping().get_reference(
        property.binding.reference_id.get().expect("import reference"),
      ).symbol_id().expect("import symbol");
      let key = ast::Str::from_str_in(ctx.scoping().symbol_name(symbol), self);
      let value = if let Some(initializer) = property.init.take() {
        ast::AssignmentTargetMaybeDefault::new_assignment_target_with_default(
          SPAN, target.into(), initializer, self,
        )
      } else {
        target.into()
      };
      *node = ast::AssignmentTargetProperty::new_assignment_target_property_property(
        SPAN,
        ast::PropertyKey::new_static_identifier(property.span, key, self),
        value,
        false,
        self,
      );
    }
  }

  fn enter_program(
    &mut self,
    node: &mut ast::Program<'ast>,
    ctx: &mut oxc_traverse::TraverseCtx<'ast, ()>,
  ) {
    let taken_body = node.body.take_in(self);
    node.body.reserve_exact(taken_body.len());
    taken_body.into_iter().for_each(|top_level_stmt| {
      self.handle_top_level_stmt(&mut node.body, top_level_stmt, ctx.scoping());
    });
  }

  fn exit_program(
    &mut self,
    node: &mut ast::Program<'ast>,
    ctx: &mut oxc_traverse::TraverseCtx<'ast, ()>,
  ) {
    let mut try_block = ast::BlockStatement::boxed(SPAN, [], self);

    // `initModule("<stable id>")` for EVERY static dep, uniformly registry-gated: a
    // co-carried factory runs, a resident module short-circuits. No payload-membership
    // split exists in the emitted bytes.
    // A dep can also carry statements of its own, emitted here before the body: a
    // `var import_dep = loadExports("dep.js")` binding, and one `__reExport(...)` copy per
    // `export * from`. Which ones it carries depends on how it was imported. The order is:
    //
    //   for each dep in `dependencies` (first-reference order):
    //     initModule(dep)
    //     if the dep has a binding: var import_dep = loadExports(dep)
    //     if any `export *` source is now ready: copy it, in `export *` order,
    //     stopping at the first one that is not ready yet
    //
    // Rule 1: as early as possible, copy a re-export (`export * from './dep.js'`) or make a name
    // readable (`export { x } from './dep.js'`, `export * as ns from './dep.js'` -
    // rolldown#10781). Example (the cycle from vitejs/vite#21626):
    //
    //   // index.js                     // b.js
    //   export * from './a.js'          import { valueA } from './index.js'
    //   export { fn } from './b.js'     export const fn = () => valueA.concat('!')
    //
    //   initModule("a.js")
    //   __reExport(exports, loadExports("a.js"))   // `valueA` is on the namespace now
    //   initModule("b.js")                         // b.js runs here and reads `valueA`
    //
    // b.js runs inside `initModule("b.js")`, before this body. If the copy of a.js came after
    // all `initModule` calls, b.js would read `valueA` as `undefined`.
    //
    // Rule 2: keep `export *` order. `__reExport` skips a name the target already has, so the
    // first copy owns a name that two sources export. Example:
    //
    //   import { helper } from './b.js'   // `dependencies` = [b, c]
    //   export * from './c.js'            // `re_export_all_dependencies` = [c, b]
    //   export * from './b.js'            // both c.js and b.js export `foo`
    //
    //   initModule("b.js")                           // c.js is not ready, so no copy yet
    //   initModule("c.js")
    //   __reExport(exports, loadExports("c.js"))     // `foo` comes from c.js
    //   __reExport(exports, loadExports("b.js"))     // `foo` already set, skipped
    //
    // Copying b.js right after `initModule("b.js")` would give `foo` to b.js, only because an
    // unrelated import mentioned b.js first.
    //
    // An external needs no `initModule`. Its `import * as` binding is hoisted, so it is always
    // ready to copy.
    let mut load_exports_stmts = std::mem::take(&mut self.generated_load_exports_stmts);
    let mut dependencies_init_fn_stmts: Vec<ast::Statement<'ast>> = Vec::new();
    let mut initialized = FxHashSet::default();
    let mut next_copy = 0;
    for dep in &self.dependencies {
      let module = &self.modules[*dep];
      if module.as_normal().is_some() {
        dependencies_init_fn_stmts.push(ast::Statement::new_expression_statement(
          SPAN,
          self.make_init_module_call(module),
          self,
        ));
      }
      if let Some(stmt) = load_exports_stmts.remove(dep) {
        dependencies_init_fn_stmts.push(stmt);
      }
      initialized.insert(*dep);
      while let Some(source) = self.re_export_all_dependencies.get_index(next_copy) {
        let source_module = &self.modules[*source];
        if source_module.as_normal().is_some() && !initialized.contains(source) {
          break;
        }
        dependencies_init_fn_stmts.push(self.create_re_export_call_stmt(source_module));
        next_copy += 1;
      }
    }
    // Every binding is created next to a `self.dependencies.insert`, so the loop above emits all
    // of them.
    debug_assert!(load_exports_stmts.is_empty());
    // The copies above are the module's star exports, after which its namespace takes no names.
    if !self.re_export_all_dependencies.is_empty()
      && matches!(self.exports_kind, rolldown_common::ExportsKind::Esm)
    {
      dependencies_init_fn_stmts.push(self.create_star_namespace_completion_stmt());
    }

    let runtime_module_register = self.generate_runtime_module_register_for_hmr(ctx.scoping());

    // Factories uniformly take only `__rolldown_module_id__`; for CommonJS the
    // module/exports objects become locals the body's rewritten `module`/`exports`
    // references resolve to.
    let cjs_module_locals: Vec<ast::Statement<'ast>> = if self.exports_kind.is_commonjs() {
      let empty_exports_object = ast::Expression::new_object_expression(SPAN, [], self);
      let module_object = ast::Expression::new_object_expression(
        SPAN,
        [ast::ObjectPropertyKind::new_object_property(
          SPAN,
          ast::PropertyKind::Init,
          ast::PropertyKey::new_static_identifier(SPAN, "exports", self),
          empty_exports_object,
          true,
          false,
          false,
          self,
        )],
        self,
      );
      vec![
        // var __rolldown_module__ = { exports: {} };
        ast::Statement::from(ast::Declaration::new_variable_declaration(
          SPAN,
          ast::VariableDeclarationKind::Var,
          oxc::allocator::Vec::from_value_in(
            ast::VariableDeclarator::new(
              SPAN,
              ast::BindingPattern::new_binding_identifier(
                SPAN,
                CJS_ROLLDOWN_MODULE_REF_IDENT,
                self,
              ),
              None,
              Some(module_object),
              false,
              self,
            ),
            self,
          ),
          false,
          self,
        )),
        // var __rolldown_exports__ = __rolldown_module__.exports;
        ast::Statement::from(ast::Declaration::new_variable_declaration(
          SPAN,
          ast::VariableDeclarationKind::Var,
          oxc::allocator::Vec::from_value_in(
            ast::VariableDeclarator::new(
              SPAN,
              ast::BindingPattern::new_binding_identifier(
                SPAN,
                CJS_ROLLDOWN_EXPORTS_REF_IDENT,
                self,
              ),
              None,
              Some(Expression::new_member_access_expr(
                CJS_ROLLDOWN_MODULE_REF,
                "exports",
                self,
              )),
              false,
              self,
            ),
            self,
          ),
          false,
          self,
        )),
      ]
    } else {
      vec![]
    };

    try_block.body.reserve_exact(
      cjs_module_locals.len()
        + runtime_module_register.len()
        + node.body.len()
        + dependencies_init_fn_stmts.len()
        + 1, /* import.meta.hot*/
    );
    try_block.body.extend(cjs_module_locals);
    try_block.body.extend(runtime_module_register);
    try_block.body.extend(dependencies_init_fn_stmts);
    try_block
      .body
      .push(self.create_module_hot_context_initializer_stmt());
    try_block.body.extend(node.body.take_in(self));

    node
      .body
      .extend(std::mem::take(&mut self.generated_static_import_stmts_from_external).into_values());

    let final_block = ast::BlockStatement::boxed(SPAN, [], self);

    let try_stmt =
      ast::Statement::new_try_statement(SPAN, try_block, None, Some(final_block), self);

    // The runtime calls the factory with the module's stable id as its argument, so it's
    // available inside the body as `__rolldown_module_id__`. This lets registerModule /
    // createModuleHotContext reference the id by identifier instead of duplicating the
    // string literal.
    let module_id_param = ast::FormalParameter::new(
      SPAN,
      [],
      ast::BindingPattern::new_binding_identifier(SPAN, MODULE_ID_PARAM_FOR_HMR, self),
      None,
      None,
      false,
      None,
      false,
      false,
      self,
    );
    let params = ast::FormalParameters::boxed(
      SPAN,
      ast::FormalParameterKind::Signature,
      [module_id_param],
      None,
      self,
    );
    let is_async = self.module.ast_usage.contains(EcmaModuleAstUsage::TopLevelAwait);
    // function () { [user code] }
    let mut user_code_wrapper = ast::Function::boxed(
      SPAN,
      ast::FunctionType::FunctionExpression,
      None,
      false,
      is_async,
      false,
      None,
      None,
      params,
      None,
      Some(ast::FunctionBody::boxed(SPAN, [], [try_stmt], self)),
      self,
    );
    // mark the callback as PIFE because the callback is executed when this chunk is loaded
    user_code_wrapper.pife = self.use_pife_for_module_wrappers;

    // __rolldown_runtime__.registerFactory(stable_id, kind, function (__rolldown_module_id__) { [user code] }[, stamp])
    // Every factory is id-addressed and registry-gated at runtime; re-execution policy
    // is runtime data (evictions), never a per-payload flag.
    let mut register_factory_args = oxc::allocator::Vec::with_capacity_in(4, self);
    register_factory_args.push(ast::Argument::new_string_literal(
      SPAN,
      oxc::ast::ast::Str::from_str_in(&self.module.stable_id, self),
      None,
      self,
    ));
    register_factory_args.push(ast::Argument::new_string_literal(
      SPAN,
      oxc::ast::ast::Str::from_str_in(
        if self.exports_kind.is_commonjs() {
          "cjs"
        } else {
          "esm"
        },
        self,
      ),
      None,
      self,
    ));
    register_factory_args
      .push(ast::Argument::from(ast::Expression::FunctionExpression(user_code_wrapper)));
    if self.stamp != 0 {
      register_factory_args.push(ast::Argument::new_numeric_literal(
        SPAN,
        f64::from(self.stamp),
        None,
        NumberBase::Decimal,
        self,
      ));
    }

    let register_factory_call = ast::Expression::new_call_expression(
      SPAN,
      Expression::new_identifier(SPAN, "__rolldown_runtime__.registerFactory", self),
      None,
      register_factory_args,
      false,
      self,
    );

    node.body.push(ast::Statement::new_expression_statement(
      SPAN,
      register_factory_call,
      self,
    ));
  }

  fn enter_call_expression(
    &mut self,
    node: &mut ast::CallExpression<'ast>,
    _ctx: &mut oxc_traverse::TraverseCtx<'ast, ()>,
  ) {
    self.rewrite_hot_accept_call_deps(node);
    crate::utils::import_options::rewrite_hmr_import_attribute_target(
      node, self.module, self.modules, self,
    );
  }

  fn exit_call_expression(
    &mut self,
    node: &mut ast::CallExpression<'ast>,
    ctx: &mut oxc_traverse::TraverseCtx<'ast, ()>,
  ) {
    self.guard_import_binding_callee(&mut node.callee, ctx);
  }

  fn exit_tagged_template_expression(
    &mut self,
    node: &mut ast::TaggedTemplateExpression<'ast>,
    ctx: &mut oxc_traverse::TraverseCtx<'ast, ()>,
  ) {
    self.guard_import_binding_callee(&mut node.tag, ctx);
  }

  fn exit_expression(
    &mut self,
    node: &mut oxc::ast::ast::Expression<'ast>,
    ctx: &mut oxc_traverse::TraverseCtx<'ast, ()>,
  ) {
    crate::utils::temporal_reads::unwrap_temporal_read(node);
    // Rewrite top-level `this` as the main build does, by the kind that the scan decided
    // (`this_expr_replace_map`, which leaves `this` inside classes and functions alone): the
    // module's `exports` for a module that the scan found CommonJS, and otherwise the
    // `context` option, or `void 0` without one, also for a module that the link makes
    // CommonJS. A factory runs as its record's method, whose `this` is the record.
    if let ast::Expression::ThisExpression(this_expr) = node
      && let Some(kind) = self.module.ecma_view.this_expr_replace_map.get(&this_expr.node_id())
    {
      *node = match kind {
        ThisExprReplaceKind::Exports => {
          Expression::new_id_ref_expr(SPAN, CJS_ROLLDOWN_EXPORTS_REF, self)
        }
        ThisExprReplaceKind::Context if self.context.is_empty() => {
          ast::Expression::new_void_0(SPAN, self)
        }
        ThisExprReplaceKind::Context => Expression::new_id_ref_expr(SPAN, self.context, self),
      };
      return;
    }

    self.try_rewrite_dynamic_import(node);
    self.try_rewrite_require(node, ctx);
    self.rewrite_import_meta_hot(node);
  }

  fn exit_identifier_reference(
    &mut self,
    node: &mut ast::IdentifierReference<'ast>,
    ctx: &mut oxc_traverse::TraverseCtx<'ast, ()>,
  ) {
    self.rewrite_identifier_reference(node, ctx);
  }
}

impl<'ast> HmrAstFinalizer<'_, 'ast> {
  fn imported_assignment_target(
    &self,
    reference: &ast::IdentifierReference<'ast>,
    ctx: &oxc_traverse::TraverseCtx<'ast, ()>,
  ) -> Option<ast::SimpleAssignmentTarget<'ast>> {
    let reference = ctx.scoping().get_reference(reference.reference_id.get()?);
    let binding = self.import_bindings.get(&reference.symbol_id()?)?;
    let expression = format!(
      "({{get value(){{return ({binding})}},set value(_){{const immutable=0;immutable=_}}}}).value"
    );
    let expression = parse_injected_expression(self.allocator(), &expression)
      .expect("generated immutable import assignment target");
    Some(ast::SimpleAssignmentTarget::from(expression.into_member_expression()))
  }

  /// Rewrite a bare `exports` / `module` identifier to the wrapper-parameter
  /// name (`__rolldown_exports__` / `__rolldown_module__`), or an import-binding
  /// identifier to its generated binding name.
  fn rewrite_identifier_reference(
    &self,
    ident: &mut ast::IdentifierReference<'ast>,
    ctx: &oxc_traverse::TraverseCtx<'ast, ()>,
  ) {
    let Some(reference_id) = ident.reference_id.get() else {
      return;
    };
    let reference = ctx.scoping().get_reference(reference_id);
    if let Some(symbol_id) = reference.symbol_id() {
      if let Some(binding_name) = self.import_bindings.get(&symbol_id) {
        ident.name = oxc::ast::ast::Str::from_str_in(binding_name.as_str(), self).into();
      }
    } else if ident.name == CJS_EXPORTS_REF_STR {
      ident.name = CJS_ROLLDOWN_EXPORTS_REF_IDENT;
    } else if ident.name == CJS_MODULE_REF_STR {
      ident.name = CJS_ROLLDOWN_MODULE_REF_IDENT;
    }
  }

  /// An import binding renamed to `import_foo.bar` or `import_foo["a-b"]` would be called as a
  /// method, with the exports object as `this`. Wrap it as `(0, import_foo.bar)` so `this` stays
  /// `undefined`. A namespace import is renamed to `import_foo` alone and is left as it is.
  fn guard_import_binding_callee(
    &self,
    callee: &mut Expression<'ast>,
    ctx: &oxc_traverse::TraverseCtx<'ast, ()>,
  ) {
    let callee = callee.without_parentheses_mut();
    let Expression::Identifier(ident) = callee else {
      return;
    };
    let Some(reference_id) = ident.reference_id.get() else {
      return;
    };
    let Some(symbol_id) = ctx.scoping().get_reference(reference_id).symbol_id() else {
      return;
    };
    if !self.import_bindings.get(&symbol_id).is_some_and(|name| name.contains(['.', '['])) {
      return;
    }
    let member = callee.take_in(self);
    *callee = Expression::new_seq_in_parens(
      Expression::new_numeric_literal(SPAN, 0.0, Some("0".into()), NumberBase::Decimal, self),
      member,
      self,
    );
  }
}
