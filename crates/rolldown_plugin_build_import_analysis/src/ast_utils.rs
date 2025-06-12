use oxc::{
  allocator::{CloneIn as _, TakeIn as _},
  ast::{
    NONE,
    ast::{
      Argument, BindingPattern, BindingPatternKind, CallExpression, Declaration, Expression,
      FormalParameterKind, Statement, StaticMemberExpression, VariableDeclarationKind,
    },
  },
  semantic::ScopeFlags,
  span::SPAN,
};
use rolldown_ecmascript_utils::{AstSnippet, BindingPatternExt as _};

use super::ast_visit::BuildImportAnalysisVisitor;

const IS_MODERN_FLAG: &str = "__VITE_IS_MODERN__";

impl<'a> BuildImportAnalysisVisitor<'a> {
  pub fn new(
    snippet: AstSnippet<'a>,
    insert_preload: bool,
    render_built_url: bool,
    is_relative_base: bool,
  ) -> Self {
    Self {
      snippet,
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
  pub fn rewrite_member_expr(&mut self, member_expr: &mut StaticMemberExpression<'a>) {
    let mut await_expr = &mut member_expr.object;
    while let Expression::ParenthesizedExpression(member_expr) = await_expr {
      await_expr = &mut member_expr.expression;
    }
    if matches!(await_expr,  Expression::AwaitExpression(expr) if matches!(expr.argument, Expression::ImportExpression(_)))
    {
      let property = match member_expr.property.name.as_str() {
        "default" => self.snippet.atom("__vite_default__"),
        _ => member_expr.property.name,
      };
      *await_expr = Expression::AwaitExpression(self.snippet.builder.alloc_await_expression(
        SPAN,
        self.construct_vite_preload_call(
          self.snippet.builder.binding_pattern(
            BindingPatternKind::ObjectPattern(self.snippet.builder.alloc_object_pattern(
              SPAN,
              self.snippet.builder.vec1(self.snippet.builder.binding_property(
                SPAN,
                self.snippet.builder.property_key_static_identifier(SPAN, property),
                self.snippet.builder.binding_pattern(
                  BindingPatternKind::BindingIdentifier(
                    self.snippet.builder.alloc_binding_identifier(SPAN, property),
                  ),
                  NONE,
                  false,
                ),
                true,
                false,
              )),
              NONE,
            )),
            NONE,
            false,
          ),
          await_expr.take_in(self.snippet.alloc()),
        ),
      ));
      self.need_prepend_helper = true;
    }
  }

  /// transform `import('foo').then(({foo})=>{})`
  /// to `__vitePreload(async () => { let foo; return {foo} = await import('foo'); },...).then(({foo})=>{})`
  pub fn rewrite_import_expr(&mut self, expr: &mut CallExpression<'a>) {
    let Expression::StaticMemberExpression(ref mut callee) = expr.callee else {
      return;
    };
    if callee.property.name != "then"
      || expr.arguments.is_empty()
      || !matches!(callee.object, Expression::ImportExpression(_))
    {
      return;
    }
    let arg = match &expr.arguments[0] {
      Argument::ArrowFunctionExpression(expr) if !expr.params.is_empty() => &expr.params.items[0],
      Argument::FunctionExpression(expr) if !expr.params.is_empty() => &expr.params.items[0],
      _ => return,
    };
    callee.object = self.construct_vite_preload_call(
      arg.pattern.clone_in(self.snippet.alloc()),
      self.snippet.builder.expression_await(SPAN, callee.object.take_in(self.snippet.alloc())),
    );
    self.need_prepend_helper = true;
  }

  pub fn construct_vite_preload_call(
    &self,
    binding_pat: BindingPattern<'a>,
    await_expr: Expression<'a>,
  ) -> Expression<'a> {
    self.snippet.builder.expression_call(
      SPAN,
      self.snippet.id_ref_expr("__vitePreload", SPAN),
      NONE,
      {
        let append_import_meta_url = self.render_built_url || self.is_relative_base;
        let capacity = if append_import_meta_url { 3 } else { 2 };
        let mut items = self.snippet.builder.vec_with_capacity(capacity);
        items.push(Argument::from(Expression::ArrowFunctionExpression(
          self.snippet.builder.alloc_arrow_function_expression(
            SPAN,
            false,
            true,
            NONE,
            self.snippet.builder.formal_parameters(
              SPAN,
              FormalParameterKind::Signature,
              self.snippet.builder.vec(),
              NONE,
            ),
            NONE,
            self.snippet.builder.function_body(SPAN, self.snippet.builder.vec(), {
              let mut statements = self.snippet.builder.vec_with_capacity(2);
              statements.push(Statement::from(Declaration::VariableDeclaration(
                self.snippet.builder.alloc_variable_declaration(
                  SPAN,
                  VariableDeclarationKind::Const,
                  self.snippet.builder.vec1(self.snippet.builder.variable_declarator(
                    SPAN,
                    VariableDeclarationKind::Const,
                    binding_pat.clone_in(self.snippet.alloc()),
                    Some(await_expr),
                    false,
                  )),
                  false,
                ),
              )));
              statements.push(Statement::ReturnStatement(
                self
                  .snippet
                  .builder
                  .alloc_return_statement(SPAN, Some(binding_pat.into_expression(&self.snippet))),
              ));
              statements
            }),
          ),
        )));
        items.push(Argument::from(self.snippet.builder.expression_conditional(
          SPAN,
          self.snippet.id_ref_expr(IS_MODERN_FLAG, SPAN),
          self.snippet.id_ref_expr("__VITE_PRELOAD__", SPAN),
          self.snippet.void_zero(),
        )));
        if append_import_meta_url {
          items.push(Argument::from(Expression::from(
            self.snippet.builder.member_expression_static(
              SPAN,
              self.snippet.builder.expression_meta_property(
                SPAN,
                self.snippet.id_name("import", SPAN),
                self.snippet.id_name("meta", SPAN),
              ),
              self.snippet.id_name("url", SPAN),
              false,
            ),
          )));
        }
        items
      },
      false,
    )
  }
}
