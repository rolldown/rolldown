use oxc::allocator::GetAllocator;
use oxc::{
  allocator::{CloneIn as _, TakeIn as _},
  ast::{
    NONE,
    ast::{
      Argument, ArrowFunctionExpression, AwaitExpression, BindingPattern, BindingProperty,
      Declaration, Expression, FormalParameterKind, FormalParameters, FunctionBody,
      MemberExpression, ObjectPattern, PropertyKey, ReturnStatement, Statement,
      StaticMemberExpression, VariableDeclaration, VariableDeclarationKind, VariableDeclarator,
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
      *await_expr = Expression::AwaitExpression(AwaitExpression::boxed(
        SPAN,
        self.construct_vite_preload_call(
          ObjectPattern::boxed(
            SPAN,
            oxc::allocator::Vec::from_value_in(
              BindingProperty::new(
                SPAN,
                PropertyKey::new_static_identifier(SPAN, key, &self.ast_factory),
                BindingPattern::new_binding_identifier(SPAN, value, &self.ast_factory),
                true,
                false,
                &self.ast_factory,
              ),
              &self.ast_factory,
            ),
            NONE,
            &self.ast_factory,
          ),
          await_expr.take_in(&self.ast_factory.allocator()),
        ),
        &self.ast_factory,
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
      match &params.items.first()?.pattern {
        BindingPattern::ObjectPattern(object_pat) => {
          Some(object_pat.clone_in(self.ast_factory.allocator()))
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
          callee.object.take_in(&self.ast_factory.allocator()),
          &self.ast_factory,
        ),
      );
      walk_arguments(self, &mut call_expr.arguments);
      return true;
    }

    // For non-destructuring: wrap the entire import().then() expression
    walk_arguments(self, &mut call_expr.arguments);
    let import_then_expr = expr.take_in(&self.ast_factory.allocator());
    *expr = self
      .vite_preload_call(Argument::from(self.ast_factory.make_arrow_returning(import_then_expr)));
    true
  }

  /// transform `import('foo')`
  /// to `__vitePreload(() => import('foo'),...)`
  pub fn rewrite_import_expr(&self, expr: &mut Expression<'a>) -> bool {
    let Expression::ImportExpression(_) = expr else { return false };
    *expr = self.vite_preload_call(Argument::from(
      self.ast_factory.make_arrow_returning(expr.take_in(&self.ast_factory.allocator())),
    ));
    true
  }

  pub fn construct_vite_preload_call(
    &self,
    object_pat: oxc::allocator::Box<'a, ObjectPattern<'a>>,
    await_expr: Expression<'a>,
  ) -> Expression<'a> {
    self.vite_preload_call(Argument::from(Expression::ArrowFunctionExpression(
      ArrowFunctionExpression::boxed(
        SPAN,
        false,
        true,
        NONE,
        FormalParameters::new(
          SPAN,
          FormalParameterKind::Signature,
          oxc::allocator::Vec::new_in(&self.ast_factory),
          NONE,
          &self.ast_factory,
        ),
        NONE,
        FunctionBody::new(
          SPAN,
          oxc::allocator::Vec::new_in(&self.ast_factory),
          {
            let mut statements = oxc::allocator::Vec::with_capacity_in(2, &self.ast_factory);
            statements.push(Statement::from(Declaration::VariableDeclaration(
              VariableDeclaration::boxed(
                SPAN,
                VariableDeclarationKind::Const,
                oxc::allocator::Vec::from_value_in(
                  VariableDeclarator::new(
                    SPAN,
                    VariableDeclarationKind::Const,
                    BindingPattern::ObjectPattern(
                      object_pat.clone_in(self.ast_factory.allocator()),
                    ),
                    NONE,
                    Some(await_expr),
                    false,
                    &self.ast_factory,
                  ),
                  &self.ast_factory,
                ),
                false,
                &self.ast_factory,
              ),
            )));
            statements.push(Statement::ReturnStatement(ReturnStatement::boxed(
              SPAN,
              Some(BindingPattern::ObjectPattern(object_pat).into_expression(&self.ast_factory)),
              &self.ast_factory,
            )));
            statements
          },
          &self.ast_factory,
        ),
        &self.ast_factory,
      ),
    )))
  }

  pub fn vite_preload_call(&self, argument: Argument<'a>) -> Expression<'a> {
    Expression::new_call_expression(
      SPAN,
      self.ast_factory.make_id_ref_expr(SPAN, "__vitePreload"),
      NONE,
      {
        let append_import_meta_url = self.render_built_url || self.is_relative_base;
        let capacity = if append_import_meta_url { 3 } else { 2 };
        let mut items = oxc::allocator::Vec::with_capacity_in(capacity, &self.ast_factory);

        items.push(argument);
        items.push(Argument::from(if self.is_modern {
          self.ast_factory.make_id_ref_expr(SPAN, "__VITE_PRELOAD__")
        } else {
          Expression::new_void_0(SPAN, &self.ast_factory)
        }));
        if append_import_meta_url {
          items.push(Argument::from(Expression::from(
            MemberExpression::new_static_member_expression(
              SPAN,
              Expression::new_meta_property(
                SPAN,
                self.ast_factory.make_id_name(SPAN, "import"),
                self.ast_factory.make_id_name(SPAN, "meta"),
                &self.ast_factory,
              ),
              self.ast_factory.make_id_name(SPAN, "url"),
              false,
              &self.ast_factory,
            ),
          )));
        }
        items
      },
      false,
      &self.ast_factory,
    )
  }
}
