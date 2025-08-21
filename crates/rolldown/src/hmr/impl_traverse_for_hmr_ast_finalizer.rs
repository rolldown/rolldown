use oxc::{
  allocator::TakeIn,
  ast::{NONE, ast},
  span::SPAN,
};
use oxc_traverse::Traverse;
use rolldown_ecmascript::{
  CJS_EXPORTS_REF_ATOM, CJS_MODULE_REF_ATOM, CJS_ROLLDOWN_EXPORTS_REF,
  CJS_ROLLDOWN_EXPORTS_REF_ATOM, CJS_ROLLDOWN_MODULE_REF_ATOM,
};
use rolldown_ecmascript_utils::{ExpressionExt, quote_expr, quote_stmts};

use crate::hmr::{hmr_ast_finalizer::HmrAstFinalizer, utils::HmrAstBuilder};

impl<'ast> Traverse<'ast, ()> for HmrAstFinalizer<'_, 'ast> {
  fn enter_program(
    &mut self,
    node: &mut ast::Program<'ast>,
    ctx: &mut oxc_traverse::TraverseCtx<'ast, ()>,
  ) {
    let taken_body = node.body.take_in(self.alloc);
    node.body.reserve_exact(taken_body.len());
    taken_body.into_iter().for_each(|top_level_stmt| {
      self.handle_top_level_stmt(&mut node.body, top_level_stmt, ctx.scoping());
    });
  }

  #[expect(clippy::too_many_lines)]
  fn exit_program(
    &mut self,
    node: &mut ast::Program<'ast>,
    ctx: &mut oxc_traverse::TraverseCtx<'ast, ()>,
  ) {
    let mut try_block =
      self.snippet.builder.alloc_block_statement(SPAN, self.snippet.builder.vec());

    let dependencies_init_fns = self
      .dependencies
      .iter()
      .filter_map(|dep| self.affected_module_idx_to_init_fn_name.get(dep))
      .map(|fn_name| format!("{fn_name}();"))
      .collect::<Vec<_>>()
      .join("\n");

    let dependencies_init_fn_stmts = quote_stmts(self.alloc, dependencies_init_fns.as_str());

    let runtime_module_register = self.generate_runtime_module_register_for_hmr(ctx.scoping());

    try_block.body.reserve_exact(
    runtime_module_register.len() + node.body.len() + dependencies_init_fn_stmts.len() + 1 /* import.meta.hot*/,
  );
    try_block.body.extend(runtime_module_register);
    try_block.body.extend(dependencies_init_fn_stmts);
    try_block.body.push(self.create_module_hot_context_initializer_stmt());
    try_block.body.extend(node.body.take_in(self.alloc));

    node
      .body
      .extend(std::mem::take(&mut self.generated_static_import_stmts_from_external).into_values());

    let final_block = self.snippet.builder.alloc_block_statement(SPAN, self.snippet.builder.vec());

    let try_stmt =
      self.snippet.builder.alloc_try_statement(SPAN, try_block, NONE, Some(final_block));

    let init_fn_name = &self.affected_module_idx_to_init_fn_name[&self.module.idx];

    let mut params = self.snippet.builder.formal_parameters(
      SPAN,
      ast::FormalParameterKind::Signature,
      self.snippet.builder.vec_with_capacity(2),
      NONE,
    );
    if self.module.exports_kind.is_commonjs() {
      params.items.push(self.snippet.builder.formal_parameter(
        SPAN,
        self.builder.vec(),
        self.snippet.builder.binding_pattern(
          ast::BindingPatternKind::BindingIdentifier(
            self.snippet.builder.alloc_binding_identifier(SPAN, CJS_ROLLDOWN_EXPORTS_REF_ATOM),
          ),
          NONE,
          false,
        ),
        None,
        false,
        false,
      ));
      params.items.push(self.snippet.builder.formal_parameter(
        SPAN,
        self.builder.vec(),
        self.snippet.builder.binding_pattern(
          ast::BindingPatternKind::BindingIdentifier(
            self.snippet.builder.alloc_binding_identifier(SPAN, CJS_ROLLDOWN_MODULE_REF_ATOM),
          ),
          NONE,
          false,
        ),
        None,
        false,
        false,
      ));
    }
    // function () { [user code] }
    let mut user_code_wrapper = self.snippet.builder.alloc_function(
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
      Some(self.snippet.builder.function_body(
        SPAN,
        self.snippet.builder.vec(),
        self.snippet.builder.vec1(ast::Statement::TryStatement(try_stmt)),
      )),
    );
    // mark the callback as PIFE because the callback is executed when this chunk is loaded
    user_code_wrapper.pife = self.use_pife_for_module_wrappers;

    let initializer_call = if self.module.exports_kind.is_commonjs() {
      // __rolldown__runtime.createCjsInitializer((function (exports, module) { [user code] }))
      self.snippet.builder.alloc_call_expression(
        SPAN,
        self.snippet.id_ref_expr("__rolldown_runtime__.createCjsInitializer", SPAN),
        NONE,
        self
          .snippet
          .builder
          .vec1(ast::Argument::from(ast::Expression::FunctionExpression(user_code_wrapper))),
        false,
      )
    } else {
      // __rolldown__runtime.createEsmInitializer((function () { [user code] }))
      self.snippet.builder.alloc_call_expression(
        SPAN,
        self.snippet.id_ref_expr("__rolldown_runtime__.createEsmInitializer", SPAN),
        NONE,
        self
          .snippet
          .builder
          .vec1(ast::Argument::from(ast::Expression::FunctionExpression(user_code_wrapper))),
        false,
      )
    };

    // var init_foo = __rolldown__runtime.createEsmInitializer((function () { [user code] }))
    let var_decl = self.snippet.builder.alloc_variable_declaration(
      SPAN,
      ast::VariableDeclarationKind::Var,
      self.snippet.builder.vec1(
        self.snippet.builder.variable_declarator(
          SPAN,
          ast::VariableDeclarationKind::Var,
          self.snippet.builder.binding_pattern(
            ast::BindingPatternKind::BindingIdentifier(
              self
                .snippet
                .builder
                .alloc_binding_identifier(SPAN, self.snippet.builder.atom(init_fn_name)),
            ),
            NONE,
            false,
          ),
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
    if matches!(node, ast::Expression::ThisExpression(_)) && ctx.is_current_scope_valid_for_tla() {
      // Rewrite this to `undefined` or `exports`
      if self.module.exports_kind.is_commonjs() {
        // Rewrite this to `exports`
        *node = quote_expr(self.alloc, CJS_ROLLDOWN_EXPORTS_REF);
      } else {
        // Rewrite this to `undefined`
        *node = quote_expr(self.alloc, "void 0");
      }
    }

    if let Some(ident) = node.as_identifier_mut() {
      if let Some(reference_id) = ident.reference_id.get() {
        let reference = ctx.scoping().get_reference(reference_id);
        if let Some(symbol_id) = reference.symbol_id() {
          if let Some(binding_name) = self.import_bindings.get(&symbol_id) {
            *node = self.snippet.id_ref_expr(binding_name.as_str(), ident.span);
            return;
          }
        } else if ident.name == CJS_EXPORTS_REF_ATOM {
          // Rewrite `exports` to `__rolldown_exports__`
          ident.name = CJS_ROLLDOWN_EXPORTS_REF_ATOM;
        } else if ident.name == CJS_MODULE_REF_ATOM {
          // Rewrite `module` to `__rolldown_module__`
          ident.name = CJS_ROLLDOWN_MODULE_REF_ATOM;
        }
      }
    }

    self.try_rewrite_dynamic_import(node);
    self.try_rewrite_require(node, ctx);
    self.rewrite_import_meta_hot(node);
  }
}

trait TraverseCtxExt<'ast> {
  fn is_current_scope_valid_for_tla(&self) -> bool;
}

impl<'ast> TraverseCtxExt<'ast> for oxc_traverse::TraverseCtx<'ast, ()> {
  fn is_current_scope_valid_for_tla(&self) -> bool {
    let scoping = self.scoping();
    scoping
      .scope_ancestors(self.current_scope_id())
      .map(|scope_id| scoping.scope_flags(scope_id))
      .all(|scope_flags| scope_flags.is_block() || scope_flags.is_top())
  }
}
