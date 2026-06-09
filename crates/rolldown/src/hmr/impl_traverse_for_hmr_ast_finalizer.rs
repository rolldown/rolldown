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
    let taken_body = node.body.take_in(self.ast_factory.allocator);
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
    let mut try_block = self.ast_factory.alloc_block_statement(SPAN, self.ast_factory.vec());

    let dependencies_init_fn_stmts: Vec<_> = self
      .dependencies
      .iter()
      .filter_map(|dep| self.affected_module_idx_to_init_fn_name.get(dep))
      .map(|fn_name| {
        let call_expr = self.ast_factory.expression_call(
          SPAN,
          self.ast_factory.expression_identifier(SPAN, self.ast_factory.str(fn_name)),
          NONE,
          self.ast_factory.vec(),
          false,
        );
        self.ast_factory.statement_expression(SPAN, call_expr)
      })
      .collect();

    let runtime_module_register = self.generate_runtime_module_register_for_hmr(ctx.scoping());

    try_block.body.reserve_exact(
    runtime_module_register.len() + node.body.len() + dependencies_init_fn_stmts.len() + 1 /* import.meta.hot*/,
  );
    try_block.body.extend(runtime_module_register);
    try_block.body.extend(dependencies_init_fn_stmts);
    try_block.body.push(self.create_module_hot_context_initializer_stmt());
    try_block.body.extend(node.body.take_in(self.ast_factory.allocator));

    node
      .body
      .extend(std::mem::take(&mut self.generated_static_import_stmts_from_external).into_values());

    let final_block = self.ast_factory.alloc_block_statement(SPAN, self.ast_factory.vec());

    let try_stmt = self.ast_factory.alloc_try_statement(SPAN, try_block, NONE, Some(final_block));

    let init_fn_name = &self.affected_module_idx_to_init_fn_name[&self.module.idx];

    // The runtime wrappers (createEsmInitializer / createCjsInitializer) call the body
    // with the module's stable id as an extra argument, so it's available inside the body
    // as `__rolldown_module_id__`. This lets registerModule / createModuleHotContext reference
    // the id by identifier instead of duplicating the string literal.
    let module_id_param = self.ast_factory.formal_parameter(
      SPAN,
      self.ast_factory.vec(),
      self.ast_factory.binding_pattern_binding_identifier(SPAN, MODULE_ID_PARAM_FOR_HMR),
      NONE,
      NONE,
      false,
      None,
      false,
      false,
    );
    let params = self.ast_factory.formal_parameters(
      SPAN,
      ast::FormalParameterKind::Signature,
      {
        if self.module.exports_kind.is_commonjs() {
          self.ast_factory.vec_from_array([
            self.ast_factory.formal_parameter(
              SPAN,
              self.ast_factory.vec(),
              self
                .ast_factory
                .binding_pattern_binding_identifier(SPAN, CJS_ROLLDOWN_EXPORTS_REF_IDENT),
              NONE,
              NONE,
              false,
              None,
              false,
              false,
            ),
            self.ast_factory.formal_parameter(
              SPAN,
              self.ast_factory.vec(),
              self
                .ast_factory
                .binding_pattern_binding_identifier(SPAN, CJS_ROLLDOWN_MODULE_REF_IDENT),
              NONE,
              NONE,
              false,
              None,
              false,
              false,
            ),
            module_id_param,
          ])
        } else {
          self.ast_factory.vec1(module_id_param)
        }
      },
      NONE,
    );
    // function () { [user code] }
    let mut user_code_wrapper = self.ast_factory.alloc_function(
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
      Some(self.ast_factory.function_body(
        SPAN,
        self.ast_factory.vec(),
        self.ast_factory.vec1(ast::Statement::TryStatement(try_stmt)),
      )),
    );
    // mark the callback as PIFE because the callback is executed when this chunk is loaded
    user_code_wrapper.pife = self.use_pife_for_module_wrappers;

    // Initializer call arguments: always (stable_id, factory). For lazy-compilation
    // chunks we append a truthy dedup flag so the runtime short-circuits re-execution
    // when another lazy blob has already registered this module. HMR patches omit the
    // flag so the runtime always re-executes the body to publish new exports.
    let mut initializer_args = self.ast_factory.vec_with_capacity(3);
    initializer_args.push(ast::Argument::StringLiteral(self.ast_factory.alloc_string_literal(
      SPAN,
      self.ast_factory.str(&self.module.stable_id),
      None,
    )));
    initializer_args
      .push(ast::Argument::from(ast::Expression::FunctionExpression(user_code_wrapper)));
    if self.dedup_module_initializer {
      initializer_args.push(ast::Argument::from(self.ast_factory.expression_numeric_literal(
        SPAN,
        1.0,
        None,
        ast::NumberBase::Decimal,
      )));
    }

    let initializer_callee = if self.module.exports_kind.is_commonjs() {
      // __rolldown__runtime.createCjsInitializer(stable_id, (function (exports, module) { [user code] })[, 1])
      "__rolldown_runtime__.createCjsInitializer"
    } else {
      // __rolldown__runtime.createEsmInitializer(stable_id, (function () { [user code] })[, 1])
      "__rolldown_runtime__.createEsmInitializer"
    };
    let initializer_call = self.ast_factory.alloc_call_expression(
      SPAN,
      self.ast_factory.expression_identifier(SPAN, self.ast_factory.str(initializer_callee)),
      NONE,
      initializer_args,
      false,
    );

    // var init_foo = __rolldown__runtime.createEsmInitializer((function () { [user code] }))
    let var_decl = self.ast_factory.alloc_variable_declaration(
      SPAN,
      ast::VariableDeclarationKind::Var,
      self.ast_factory.vec1(
        self.ast_factory.variable_declarator(
          SPAN,
          ast::VariableDeclarationKind::Var,
          self
            .ast_factory
            .binding_pattern_binding_identifier(SPAN, self.ast_factory.str(init_fn_name)),
          NONE,
          Some(ast::Expression::CallExpression(initializer_call)),
          false,
        ),
      ),
      false,
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
      && self.module.ecma_view.this_expr_replace_map.contains_key(&this_expr.span)
    {
      *node = self
        .ast_factory
        .expression_identifier(SPAN, self.ast_factory.str(CJS_ROLLDOWN_EXPORTS_REF));
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
        ident.name = self.ast_factory.str(binding_name.as_str()).into();
      }
    } else if ident.name == CJS_EXPORTS_REF_STR {
      ident.name = CJS_ROLLDOWN_EXPORTS_REF_IDENT;
    } else if ident.name == CJS_MODULE_REF_STR {
      ident.name = CJS_ROLLDOWN_MODULE_REF_IDENT;
    }
  }
}
