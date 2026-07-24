use oxc::allocator::GetAllocator;
use oxc::{
  allocator::TakeIn,
  ast::{
    ast::{self, Expression},
    builder::NONE,
  },
  span::SPAN,
};
use oxc_traverse::Traverse;
use rolldown_ecmascript::{
  CJS_EXPORTS_REF_STR, CJS_MODULE_REF_STR, CJS_ROLLDOWN_EXPORTS_REF,
  CJS_ROLLDOWN_EXPORTS_REF_IDENT, CJS_ROLLDOWN_MODULE_REF, CJS_ROLLDOWN_MODULE_REF_IDENT,
};
use rolldown_ecmascript_utils::ExpressionFactoryExt as _;

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
    let taken_body = node.body.take_in(&self.ast_builder.allocator());
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
    let mut try_block = ast::BlockStatement::boxed(
      SPAN,
      oxc::allocator::Vec::new_in(&self.ast_builder),
      &self.ast_builder,
    );

    // `initModule("<stable id>")` for EVERY static dep, uniformly registry-gated: a
    // co-carried factory runs, a resident module short-circuits. No payload-membership
    // split exists in the emitted bytes.
    let dependencies_init_fn_stmts: Vec<_> = self
      .dependencies
      .iter()
      .filter_map(|dep| {
        let module = &self.modules[*dep];
        module.as_normal().map(|_| {
          ast::Statement::new_expression_statement(
            SPAN,
            self.make_init_module_call(module),
            &self.ast_builder,
          )
        })
      })
      .collect();

    let runtime_module_register = self.generate_runtime_module_register_for_hmr(ctx.scoping());

    // Factories uniformly take only `__rolldown_module_id__`; for CommonJS the
    // module/exports objects become locals the body's rewritten `module`/`exports`
    // references resolve to.
    let cjs_module_locals: Vec<ast::Statement<'ast>> = if self.module.exports_kind.is_commonjs() {
      let empty_exports_object = ast::Expression::new_object_expression(
        SPAN,
        oxc::allocator::Vec::new_in(&self.ast_builder),
        &self.ast_builder,
      );
      let module_object = ast::Expression::new_object_expression(
        SPAN,
        oxc::allocator::Vec::from_value_in(
          ast::ObjectPropertyKind::new_object_property(
            SPAN,
            ast::PropertyKind::Init,
            ast::PropertyKey::new_static_identifier(SPAN, "exports", &self.ast_builder),
            empty_exports_object,
            true,
            false,
            false,
            &self.ast_builder,
          ),
          &self.ast_builder,
        ),
        &self.ast_builder,
      );
      vec![
        // var __rolldown_module__ = { exports: {} };
        ast::Statement::from(ast::Declaration::new_variable_declaration(
          SPAN,
          ast::VariableDeclarationKind::Var,
          oxc::allocator::Vec::from_value_in(
            ast::VariableDeclarator::new(
              SPAN,
              ast::VariableDeclarationKind::Var,
              ast::BindingPattern::new_binding_identifier(
                SPAN,
                CJS_ROLLDOWN_MODULE_REF_IDENT,
                &self.ast_builder,
              ),
              NONE,
              Some(module_object),
              false,
              &self.ast_builder,
            ),
            &self.ast_builder,
          ),
          false,
          &self.ast_builder,
        )),
        // var __rolldown_exports__ = __rolldown_module__.exports;
        ast::Statement::from(ast::Declaration::new_variable_declaration(
          SPAN,
          ast::VariableDeclarationKind::Var,
          oxc::allocator::Vec::from_value_in(
            ast::VariableDeclarator::new(
              SPAN,
              ast::VariableDeclarationKind::Var,
              ast::BindingPattern::new_binding_identifier(
                SPAN,
                CJS_ROLLDOWN_EXPORTS_REF_IDENT,
                &self.ast_builder,
              ),
              NONE,
              Some(Expression::new_member_access_expr(
                CJS_ROLLDOWN_MODULE_REF,
                "exports",
                &self.ast_builder,
              )),
              false,
              &self.ast_builder,
            ),
            &self.ast_builder,
          ),
          false,
          &self.ast_builder,
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
    try_block.body.extend(node.body.take_in(&self.ast_builder.allocator()));

    node
      .body
      .extend(std::mem::take(&mut self.generated_static_import_stmts_from_external).into_values());

    let final_block = ast::BlockStatement::boxed(
      SPAN,
      oxc::allocator::Vec::new_in(&self.ast_builder),
      &self.ast_builder,
    );

    let try_stmt = ast::Statement::new_try_statement(
      SPAN,
      try_block,
      NONE,
      Some(final_block),
      &self.ast_builder,
    );

    // The runtime calls the factory with the module's stable id as its argument, so it's
    // available inside the body as `__rolldown_module_id__`. This lets registerModule /
    // createModuleHotContext reference the id by identifier instead of duplicating the
    // string literal.
    let module_id_param = ast::FormalParameter::new(
      SPAN,
      oxc::allocator::Vec::new_in(&self.ast_builder),
      ast::BindingPattern::new_binding_identifier(SPAN, MODULE_ID_PARAM_FOR_HMR, &self.ast_builder),
      NONE,
      NONE,
      false,
      None,
      false,
      false,
      &self.ast_builder,
    );
    let params = ast::FormalParameters::new(
      SPAN,
      ast::FormalParameterKind::Signature,
      oxc::allocator::Vec::from_value_in(module_id_param, &self.ast_builder),
      NONE,
      &self.ast_builder,
    );
    // function () { [user code] }
    let mut user_code_wrapper = ast::Function::boxed(
      SPAN,
      ast::FunctionType::FunctionExpression,
      None,
      false,
      false,
      false,
      NONE,
      NONE,
      params,
      NONE,
      Some(ast::FunctionBody::new(
        SPAN,
        oxc::allocator::Vec::new_in(&self.ast_builder),
        oxc::allocator::Vec::from_value_in(try_stmt, &self.ast_builder),
        &self.ast_builder,
      )),
      &self.ast_builder,
    );
    // mark the callback as PIFE because the callback is executed when this chunk is loaded
    user_code_wrapper.pife = self.use_pife_for_module_wrappers;

    // __rolldown_runtime__.registerFactory(stable_id, kind, function (__rolldown_module_id__) { [user code] })
    // Every factory is id-addressed and registry-gated at runtime; re-execution policy
    // is runtime data (evictions), never a per-payload flag.
    let mut register_factory_args = oxc::allocator::Vec::with_capacity_in(3, &self.ast_builder);
    register_factory_args.push(ast::Argument::new_string_literal(
      SPAN,
      oxc::ast::ast::Str::from_str_in(&self.module.stable_id, &self.ast_builder),
      None,
      &self.ast_builder,
    ));
    register_factory_args.push(ast::Argument::new_string_literal(
      SPAN,
      oxc::ast::ast::Str::from_str_in(
        if self.module.exports_kind.is_commonjs() { "cjs" } else { "esm" },
        &self.ast_builder,
      ),
      None,
      &self.ast_builder,
    ));
    register_factory_args
      .push(ast::Argument::from(ast::Expression::FunctionExpression(user_code_wrapper)));

    let register_factory_call = ast::Expression::new_call_expression(
      SPAN,
      Expression::new_id_ref_expr(SPAN, "__rolldown_runtime__.registerFactory", &self.ast_builder),
      NONE,
      register_factory_args,
      false,
      &self.ast_builder,
    );

    node.body.push(ast::Statement::new_expression_statement(
      SPAN,
      register_factory_call,
      &self.ast_builder,
    ));
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
      *node = Expression::new_id_ref_expr(SPAN, CJS_ROLLDOWN_EXPORTS_REF, &self.ast_builder);
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
        ident.name =
          oxc::ast::ast::Str::from_str_in(binding_name.as_str(), &self.ast_builder).into();
      }
    } else if ident.name == CJS_EXPORTS_REF_STR {
      ident.name = CJS_ROLLDOWN_EXPORTS_REF_IDENT;
    } else if ident.name == CJS_MODULE_REF_STR {
      ident.name = CJS_ROLLDOWN_MODULE_REF_IDENT;
    }
  }
}
