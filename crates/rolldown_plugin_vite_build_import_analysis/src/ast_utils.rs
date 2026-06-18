use oxc::{
  allocator::{CloneIn as _, TakeIn as _},
  ast::{
    NONE,
    ast::{
      Argument, BindingPattern, Declaration, Expression, FormalParameterKind, Statement,
      StaticMemberExpression, VariableDeclarationKind,
    },
  },
  ast_visit::walk_mut::walk_arguments,
  semantic::ScopeFlags,
  span::SPAN,
};
use rolldown_ecmascript_utils::{AstFactory, BindingPatternExt as _};

use super::ast_visit::BuildImportAnalysisVisitor;

impl<'a> BuildImportAnalysisVisitor<'a> {
  #[expect(clippy::fn_params_excessive_bools)]
  pub fn new(
    ast_factory: AstFactory<'a>,
    insert_preload: bool,
    render_built_url: bool,
    is_relative_base: bool,
    is_modern: bool,
  ) -> Self {
    Self {
      ast_factory,
      is_modern,
      insert_preload,
      render_built_url,
      is_relative_base,
      scope_stack: vec![],
      need_prepend_helper: false,
      has_inserted_helper: false,
    }
  }

  #[inline]
  pub fn is_top_level(&self) -> bool {
    self.scope_stack.last().is_some_and(|flags| flags.contains(ScopeFlags::Top))
  }

  /// transform `(await import('foo')).foo`
  /// to `(await __vitePreload(async () => { let foo; return {foo} = await import('foo'); },...))).foo`
  pub fn rewrite_member_expr(&self, member_expr: &mut StaticMemberExpression<'a>) -> bool {
    let mut await_expr = &mut member_expr.object;
    while let Expression::ParenthesizedExpression(member_expr) = await_expr {
      await_expr = &mut member_expr.expression;
    }
    if matches!(await_expr,  Expression::AwaitExpression(expr) if matches!(expr.argument, Expression::ImportExpression(_)))
    {
      let (key, value) = match member_expr.property.name.as_str() {
        // avoid `default` keyword error
        key @ "default" => (key, "__vite_default__"),
        _ => (member_expr.property.name.as_str(), member_expr.property.name.as_str()),
      };
      *await_expr = Expression::AwaitExpression(self.ast_factory.alloc_await_expression(
        SPAN,
        self.construct_vite_preload_call(
          BindingPattern::ObjectPattern(self.ast_factory.alloc_object_pattern(
            SPAN,
            self.ast_factory.vec1(self.ast_factory.binding_property(
              SPAN,
              self.ast_factory.property_key_static_identifier(SPAN, key),
              self.ast_factory.binding_pattern_binding_identifier(SPAN, value),
              true,
              false,
            )),
            NONE,
          )),
          await_expr.take_in(self.ast_factory.allocator),
        ),
      ));
      return true;
    }
    false
  }

  /// transform `import('foo').then(({foo})=>{})`
  /// to `__vitePreload(async () => { let foo; return {foo} = await import('foo'); },...).then(({foo})=>{})`
  ///
  /// transform `import('foo').then((m) => m.prop)`
  /// to `__vitePreload(() => import('foo').then((m) => m.prop), ...)`
  pub fn rewrite_call_expr(&mut self, expr: &mut Expression<'a>) -> bool {
    // import(...).then(...)
    let Expression::CallExpression(call_expr) = expr else {
      return false;
    };
    let Expression::StaticMemberExpression(ref callee) = call_expr.callee else {
      return false;
    };
    if callee.property.name != "then" || !matches!(callee.object, Expression::ImportExpression(_)) {
      return false;
    }

    // Check if the .then() callback has a destructuring (ObjectPattern) parameter
    let destructuring_pat = call_expr.arguments.first().and_then(|arg| {
      let params = match arg {
        Argument::ArrowFunctionExpression(func) => &func.params,
        Argument::FunctionExpression(func) => &func.params,
        _ => return None,
      };
      let first_param = params.items.first()?;
      if matches!(&first_param.pattern, BindingPattern::ObjectPattern(_)) {
        Some(first_param.pattern.clone_in(self.ast_factory.allocator))
      } else {
        None
      }
    });
    if let Some(binding_pat) = destructuring_pat {
      // For destructuring: replace only the import() in the callee with __vitePreload(...)
      // keeping the .then() call on the outside
      let Expression::StaticMemberExpression(callee) = &mut call_expr.callee else {
        unreachable!();
      };
      callee.object = self.construct_vite_preload_call(
        binding_pat,
        self.ast_factory.expression_await(SPAN, callee.object.take_in(self.ast_factory.allocator)),
      );
      walk_arguments(self, &mut call_expr.arguments);
      return true;
    }

    // For non-destructuring: wrap the entire import().then() expression
    walk_arguments(self, &mut call_expr.arguments);
    let import_then_expr = expr.take_in(self.ast_factory.allocator);
    *expr = self
      .vite_preload_call(Argument::from(self.ast_factory.make_arrow_returning(import_then_expr)));
    true
  }

  /// transform `import('foo')`
  /// to `__vitePreload(() => import('foo'),...)`
  pub fn rewrite_import_expr(&self, expr: &mut Expression<'a>) -> bool {
    let Expression::ImportExpression(_) = expr else { return false };
    *expr = self.vite_preload_call(Argument::from(
      self.ast_factory.make_arrow_returning(expr.take_in(self.ast_factory.allocator)),
    ));
    true
  }

  pub fn construct_vite_preload_call(
    &self,
    binding_pat: BindingPattern<'a>,
    await_expr: Expression<'a>,
  ) -> Expression<'a> {
    let argument = if let BindingPattern::BindingIdentifier(_) = binding_pat {
      let Expression::AwaitExpression(expr) = await_expr else {
        unreachable!("The `await_expr` must be `Expression::AwaitExpression`.")
      };
      self.ast_factory.make_arrow_returning(expr.unbox().argument)
    } else {
      Expression::ArrowFunctionExpression(self.ast_factory.alloc_arrow_function_expression(
        SPAN,
        false,
        true,
        NONE,
        self.ast_factory.formal_parameters(
          SPAN,
          FormalParameterKind::Signature,
          self.ast_factory.vec(),
          NONE,
        ),
        NONE,
        self.ast_factory.function_body(SPAN, self.ast_factory.vec(), {
          let mut statements = self.ast_factory.vec_with_capacity(2);
          statements.push(Statement::from(Declaration::VariableDeclaration(
            self.ast_factory.alloc_variable_declaration(
              SPAN,
              VariableDeclarationKind::Const,
              self.ast_factory.vec1(self.ast_factory.variable_declarator(
                SPAN,
                VariableDeclarationKind::Const,
                binding_pat.clone_in(self.ast_factory.allocator),
                NONE,
                Some(await_expr),
                false,
              )),
              false,
            ),
          )));
          statements.push(Statement::ReturnStatement(
            self
              .ast_factory
              .alloc_return_statement(SPAN, Some(binding_pat.into_expression(&self.ast_factory))),
          ));
          statements
        }),
      ))
    };
    self.vite_preload_call(Argument::from(argument))
  }

  pub fn vite_preload_call(&self, argument: Argument<'a>) -> Expression<'a> {
    self.ast_factory.expression_call(
      SPAN,
      self.ast_factory.make_id_ref_expr(SPAN, "__vitePreload"),
      NONE,
      {
        let append_import_meta_url = self.render_built_url || self.is_relative_base;
        let capacity = if append_import_meta_url { 3 } else { 2 };
        let mut items = self.ast_factory.vec_with_capacity(capacity);

        items.push(argument);
        items.push(Argument::from(if self.is_modern {
          self.ast_factory.make_id_ref_expr(SPAN, "__VITE_PRELOAD__")
        } else {
          self.ast_factory.void_0(SPAN)
        }));
        if append_import_meta_url {
          items.push(Argument::from(Expression::from(self.ast_factory.member_expression_static(
            SPAN,
            self.ast_factory.expression_meta_property(
              SPAN,
              self.ast_factory.make_id_name(SPAN, "import"),
              self.ast_factory.make_id_name(SPAN, "meta"),
            ),
            self.ast_factory.make_id_name(SPAN, "url"),
            false,
          ))));
        }
        items
      },
      false,
    )
  }
}
