use oxc::{
  allocator::TakeIn,
  ast::ast::{self, Expression},
  span::SPAN,
};
use oxc_traverse::Traverse;
use rolldown_ecmascript::{
  CJS_EXPORTS_REF_STR, CJS_MODULE_REF_STR, CJS_ROLLDOWN_EXPORTS_REF,
  CJS_ROLLDOWN_EXPORTS_REF_IDENT, CJS_ROLLDOWN_MODULE_REF, CJS_ROLLDOWN_MODULE_REF_IDENT,
};
use rolldown_ecmascript_utils::ExpressionFactoryExt as _;
use rustc_hash::FxHashSet;

use crate::hmr::{
  hmr_ast_finalizer::HmrAstFinalizer,
  utils::{HmrAstBuilder, MODULE_ID_PARAM_FOR_HMR},
};

impl<'ast> Traverse<'ast, ()> for HmrAstFinalizer<'_, 'ast> {
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

    let runtime_module_register = self.generate_runtime_module_register_for_hmr(ctx.scoping());

    // Factories uniformly take only `__rolldown_module_id__`; for CommonJS the
    // module/exports objects become locals the body's rewritten `module`/`exports`
    // references resolve to.
    let cjs_module_locals: Vec<ast::Statement<'ast>> = if self.module.exports_kind.is_commonjs() {
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
              Some(Expression::new_member_access_expr(CJS_ROLLDOWN_MODULE_REF, "exports", self)),
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
    try_block.body.push(self.create_module_hot_context_initializer_stmt());
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
    // function () { [user code] }
    let mut user_code_wrapper = ast::Function::boxed(
      SPAN,
      ast::FunctionType::FunctionExpression,
      None,
      false,
      false,
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

    // __rolldown_runtime__.registerFactory(stable_id, kind, function (__rolldown_module_id__) { [user code] })
    // Every factory is id-addressed and registry-gated at runtime; re-execution policy
    // is runtime data (evictions), never a per-payload flag.
    let mut register_factory_args = oxc::allocator::Vec::with_capacity_in(3, self);
    register_factory_args.push(ast::Argument::new_string_literal(
      SPAN,
      oxc::ast::ast::Str::from_str_in(&self.module.stable_id, self),
      None,
      self,
    ));
    register_factory_args.push(ast::Argument::new_string_literal(
      SPAN,
      oxc::ast::ast::Str::from_str_in(
        if self.module.exports_kind.is_commonjs() { "cjs" } else { "esm" },
        self,
      ),
      None,
      self,
    ));
    register_factory_args
      .push(ast::Argument::from(ast::Expression::FunctionExpression(user_code_wrapper)));

    let register_factory_call = ast::Expression::new_call_expression(
      SPAN,
      Expression::new_identifier(SPAN, "__rolldown_runtime__.registerFactory", self),
      None,
      register_factory_args,
      false,
      self,
    );

    node.body.push(ast::Statement::new_expression_statement(SPAN, register_factory_call, self));
  }

  fn enter_call_expression(
    &mut self,
    node: &mut ast::CallExpression<'ast>,
    _ctx: &mut oxc_traverse::TraverseCtx<'ast, ()>,
  ) {
    self.rewrite_hot_accept_call_deps(node);
  }

  fn exit_expression(
    &mut self,
    node: &mut oxc::ast::ast::Expression<'ast>,
    ctx: &mut oxc_traverse::TraverseCtx<'ast, ()>,
  ) {
    // Rewrite top-level `this` to `exports` for CommonJS modules
    // Use `this_expr_replace_map` from scanning to avoid rewriting `this` inside classes
    if let ast::Expression::ThisExpression(this_expr) = node
      && self.module.exports_kind.is_commonjs()
      && self.module.ecma_view.this_expr_replace_map.contains_key(&this_expr.node_id())
    {
      *node = Expression::new_id_ref_expr(SPAN, CJS_ROLLDOWN_EXPORTS_REF, self);
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
}
