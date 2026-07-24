use oxc::allocator::GetAllocator;
use oxc::ast::builder::AstBuilder;
use oxc::{
  allocator::{CloneIn as _, TakeIn as _},
  ast::{
    ast::{
      Argument, BindingPattern, BindingProperty, Expression, FormalParameterKind, FormalParameters,
      FunctionBody, IdentifierName, ObjectPattern, PropertyKey, Statement, StaticMemberExpression,
      VariableDeclarationKind, VariableDeclarator,
    },
    builder::NONE,
  },
  ast_visit::walk_mut::walk_arguments,
  semantic::ScopeFlags,
  span::SPAN,
};
use rolldown_ecmascript_utils::{
  BindingPatternExt as _, ExpressionFactoryExt as _, IdentifierNameFactoryExt as _,
};

use super::ast_visit::BuildImportAnalysisVisitor;

impl<'a> BuildImportAnalysisVisitor<'a> {
  #[expect(clippy::fn_params_excessive_bools)]
  pub fn new(
    ast_builder: AstBuilder<'a>,
    insert_preload: bool,
    render_built_url: bool,
    is_relative_base: bool,
    is_modern: bool,
  ) -> Self {
    Self {
      ast_builder,
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
      *await_expr = Expression::new_await_expression(
        SPAN,
        self.construct_vite_preload_call(
          ObjectPattern::boxed(
            SPAN,
            oxc::allocator::Vec::from_value_in(
              BindingProperty::new(
                SPAN,
                PropertyKey::new_static_identifier(SPAN, key, &self.ast_builder),
                BindingPattern::new_binding_identifier(SPAN, value, &self.ast_builder),
                true,
                false,
                &self.ast_builder,
              ),
              &self.ast_builder,
            ),
            NONE,
            &self.ast_builder,
          ),
          await_expr.take_in(&self.ast_builder.allocator()),
        ),
        &self.ast_builder,
      );
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
      match &params.items.first()?.pattern {
        BindingPattern::ObjectPattern(object_pat) => {
          Some(object_pat.clone_in(self.ast_builder.allocator()))
        }
        _ => None,
      }
    });
    if let Some(object_pat) = destructuring_pat {
      // For destructuring: replace only the import() in the callee with __vitePreload(...)
      // keeping the .then() call on the outside
      let Expression::StaticMemberExpression(callee) = &mut call_expr.callee else {
        unreachable!();
      };
      callee.object = self.construct_vite_preload_call(
        object_pat,
        Expression::new_await_expression(
          SPAN,
          callee.object.take_in(&self.ast_builder.allocator()),
          &self.ast_builder,
        ),
      );
      walk_arguments(self, &mut call_expr.arguments);
      return true;
    }

    // For non-destructuring: wrap the entire import().then() expression
    walk_arguments(self, &mut call_expr.arguments);
    let import_then_expr = expr.take_in(&self.ast_builder.allocator());
    *expr = self.vite_preload_call(Argument::from(Expression::new_arrow_returning(
      import_then_expr,
      &self.ast_builder,
    )));
    true
  }

  /// transform `import('foo')`
  /// to `__vitePreload(() => import('foo'),...)`
  pub fn rewrite_import_expr(&self, expr: &mut Expression<'a>) -> bool {
    let Expression::ImportExpression(_) = expr else { return false };
    *expr = self.vite_preload_call(Argument::from(Expression::new_arrow_returning(
      expr.take_in(&self.ast_builder.allocator()),
      &self.ast_builder,
    )));
    true
  }

  pub fn construct_vite_preload_call(
    &self,
    object_pat: oxc::allocator::Box<'a, ObjectPattern<'a>>,
    await_expr: Expression<'a>,
  ) -> Expression<'a> {
    self.vite_preload_call(Argument::new_arrow_function_expression(
      SPAN,
      false,
      true,
      NONE,
      FormalParameters::new(
        SPAN,
        FormalParameterKind::Signature,
        oxc::allocator::Vec::new_in(&self.ast_builder),
        NONE,
        &self.ast_builder,
      ),
      NONE,
      FunctionBody::new(
        SPAN,
        oxc::allocator::Vec::new_in(&self.ast_builder),
        {
          let mut statements = oxc::allocator::Vec::with_capacity_in(2, &self.ast_builder);
          statements.push(Statement::new_variable_declaration(
            SPAN,
            VariableDeclarationKind::Const,
            oxc::allocator::Vec::from_value_in(
              VariableDeclarator::new(
                SPAN,
                VariableDeclarationKind::Const,
                BindingPattern::ObjectPattern(object_pat.clone_in(self.ast_builder.allocator())),
                NONE,
                Some(await_expr),
                false,
                &self.ast_builder,
              ),
              &self.ast_builder,
            ),
            false,
            &self.ast_builder,
          ));
          statements.push(Statement::new_return_statement(
            SPAN,
            Some(BindingPattern::ObjectPattern(object_pat).into_expression(&self.ast_builder)),
            &self.ast_builder,
          ));
          statements
        },
        &self.ast_builder,
      ),
      &self.ast_builder,
    ))
  }

  pub fn vite_preload_call(&self, argument: Argument<'a>) -> Expression<'a> {
    Expression::new_call_expression(
      SPAN,
      Expression::new_id_ref_expr(SPAN, "__vitePreload", &self.ast_builder),
      NONE,
      {
        let append_import_meta_url = self.render_built_url || self.is_relative_base;
        let capacity = if append_import_meta_url { 3 } else { 2 };
        let mut items = oxc::allocator::Vec::with_capacity_in(capacity, &self.ast_builder);

        items.push(argument);
        items.push(Argument::from(if self.is_modern {
          Expression::new_id_ref_expr(SPAN, "__VITE_PRELOAD__", &self.ast_builder)
        } else {
          Expression::new_void_0(SPAN, &self.ast_builder)
        }));
        if append_import_meta_url {
          items.push(Argument::new_static_member_expression(
            SPAN,
            Expression::new_import_meta(SPAN, &self.ast_builder),
            IdentifierName::new_id_name(SPAN, "url", &self.ast_builder),
            false,
            &self.ast_builder,
          ));
        }
        items
      },
      false,
      &self.ast_builder,
    )
  }
}
