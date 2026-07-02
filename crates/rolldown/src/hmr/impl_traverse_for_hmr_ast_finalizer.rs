use oxc::allocator::GetAllocator;
use oxc::{
  allocator::TakeIn,
  ast::{NONE, ast},
  span::SPAN,
};
use oxc_traverse::Traverse;
use rolldown_ecmascript::{
  CJS_EXPORTS_REF_STR, CJS_MODULE_REF_STR, CJS_ROLLDOWN_EXPORTS_REF,
  CJS_ROLLDOWN_EXPORTS_REF_IDENT, CJS_ROLLDOWN_MODULE_REF_IDENT,
};

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
    let taken_body = node.body.take_in(&self.ast_factory.allocator());
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
      oxc::allocator::Vec::new_in(&self.ast_factory),
      &self.ast_factory,
    );

    let dependencies_init_fn_stmts: Vec<_> = self
      .dependencies
      .iter()
      .filter_map(|dep| self.affected_module_idx_to_init_fn_name.get(dep))
      .map(|fn_name| {
        let call_expr = ast::Expression::new_call_expression(
          SPAN,
          self.ast_factory.make_id_ref_expr(SPAN, fn_name),
          NONE,
          oxc::allocator::Vec::new_in(&self.ast_factory),
          false,
          &self.ast_factory,
        );
        ast::Statement::new_expression_statement(SPAN, call_expr, &self.ast_factory)
      })
      .collect();

    let runtime_module_register = self.generate_runtime_module_register_for_hmr(ctx.scoping());

    try_block.body.reserve_exact(
    runtime_module_register.len() + node.body.len() + dependencies_init_fn_stmts.len() + 1 /* import.meta.hot*/,
  );
    try_block.body.extend(runtime_module_register);
    try_block.body.extend(dependencies_init_fn_stmts);
    try_block.body.push(self.create_module_hot_context_initializer_stmt());
    try_block.body.extend(node.body.take_in(&self.ast_factory.allocator()));

    node
      .body
      .extend(std::mem::take(&mut self.generated_static_import_stmts_from_external).into_values());

    let final_block = ast::BlockStatement::boxed(
      SPAN,
      oxc::allocator::Vec::new_in(&self.ast_factory),
      &self.ast_factory,
    );

    let try_stmt =
      ast::TryStatement::boxed(SPAN, try_block, NONE, Some(final_block), &self.ast_factory);

    let init_fn_name = &self.affected_module_idx_to_init_fn_name[&self.module.idx];

    // The runtime wrappers (createEsmInitializer / createCjsInitializer) call the body
    // with the module's stable id as an extra argument, so it's available inside the body
    // as `__rolldown_module_id__`. This lets registerModule / createModuleHotContext reference
    // the id by identifier instead of duplicating the string literal.
    let module_id_param = ast::FormalParameter::new(
      SPAN,
      oxc::allocator::Vec::new_in(&self.ast_factory),
      ast::BindingPattern::new_binding_identifier(SPAN, MODULE_ID_PARAM_FOR_HMR, &self.ast_factory),
      NONE,
      NONE,
      false,
      None,
      false,
      false,
      &self.ast_factory,
    );
    let params = ast::FormalParameters::new(
      SPAN,
      ast::FormalParameterKind::Signature,
      {
        if self.module.exports_kind.is_commonjs() {
          oxc::allocator::Vec::from_array_in(
            [
              ast::FormalParameter::new(
                SPAN,
                oxc::allocator::Vec::new_in(&self.ast_factory),
                ast::BindingPattern::new_binding_identifier(
                  SPAN,
                  CJS_ROLLDOWN_EXPORTS_REF_IDENT,
                  &self.ast_factory,
                ),
                NONE,
                NONE,
                false,
                None,
                false,
                false,
                &self.ast_factory,
              ),
              ast::FormalParameter::new(
                SPAN,
                oxc::allocator::Vec::new_in(&self.ast_factory),
                ast::BindingPattern::new_binding_identifier(
                  SPAN,
                  CJS_ROLLDOWN_MODULE_REF_IDENT,
                  &self.ast_factory,
                ),
                NONE,
                NONE,
                false,
                None,
                false,
                false,
                &self.ast_factory,
              ),
              module_id_param,
            ],
            &self.ast_factory,
          )
        } else {
          oxc::allocator::Vec::from_value_in(module_id_param, &self.ast_factory)
        }
      },
      NONE,
      &self.ast_factory,
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
        oxc::allocator::Vec::new_in(&self.ast_factory),
        oxc::allocator::Vec::from_value_in(
          ast::Statement::TryStatement(try_stmt),
          &self.ast_factory,
        ),
        &self.ast_factory,
      )),
      &self.ast_factory,
    );
    // mark the callback as PIFE because the callback is executed when this chunk is loaded
    user_code_wrapper.pife = self.use_pife_for_module_wrappers;

    // Initializer call arguments: always (stable_id, factory). For lazy-compilation
    // chunks we append a truthy dedup flag so the runtime short-circuits re-execution
    // when another lazy blob has already registered this module. HMR patches omit the
    // flag so the runtime always re-executes the body to publish new exports.
    let mut initializer_args = oxc::allocator::Vec::with_capacity_in(3, &self.ast_factory);
    initializer_args.push(ast::Argument::StringLiteral(ast::StringLiteral::boxed(
      SPAN,
      oxc::ast::ast::Str::from_str_in(&self.module.stable_id, &self.ast_factory),
      None,
      &self.ast_factory,
    )));
    initializer_args
      .push(ast::Argument::from(ast::Expression::FunctionExpression(user_code_wrapper)));
    if self.dedup_module_initializer {
      initializer_args.push(ast::Argument::from(ast::Expression::new_numeric_literal(
        SPAN,
        1.0,
        None,
        ast::NumberBase::Decimal,
        &self.ast_factory,
      )));
    }

    let initializer_callee = if self.module.exports_kind.is_commonjs() {
      // __rolldown__runtime.createCjsInitializer(stable_id, (function (exports, module) { [user code] })[, 1])
      "__rolldown_runtime__.createCjsInitializer"
    } else {
      // __rolldown__runtime.createEsmInitializer(stable_id, (function () { [user code] })[, 1])
      "__rolldown_runtime__.createEsmInitializer"
    };
    let initializer_call = ast::CallExpression::boxed(
      SPAN,
      self.ast_factory.make_id_ref_expr(SPAN, initializer_callee),
      NONE,
      initializer_args,
      false,
      &self.ast_factory,
    );

    // var init_foo = __rolldown__runtime.createEsmInitializer((function () { [user code] }))
    let var_decl = ast::VariableDeclaration::boxed(
      SPAN,
      ast::VariableDeclarationKind::Var,
      oxc::allocator::Vec::from_value_in(
        ast::VariableDeclarator::new(
          SPAN,
          ast::VariableDeclarationKind::Var,
          ast::BindingPattern::new_binding_identifier(
            SPAN,
            oxc::ast::ast::Str::from_str_in(init_fn_name, &self.ast_factory),
            &self.ast_factory,
          ),
          NONE,
          Some(ast::Expression::CallExpression(initializer_call)),
          false,
          &self.ast_factory,
        ),
        &self.ast_factory,
      ),
      false,
      &self.ast_factory,
    );

    node.body.push(ast::Statement::VariableDeclaration(var_decl));
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
      *node = self.ast_factory.make_id_ref_expr(SPAN, CJS_ROLLDOWN_EXPORTS_REF);
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
          oxc::ast::ast::Str::from_str_in(binding_name.as_str(), &self.ast_factory).into();
      }
    } else if ident.name == CJS_EXPORTS_REF_STR {
      ident.name = CJS_ROLLDOWN_EXPORTS_REF_IDENT;
    } else if ident.name == CJS_MODULE_REF_STR {
      ident.name = CJS_ROLLDOWN_MODULE_REF_IDENT;
    }
  }
}
