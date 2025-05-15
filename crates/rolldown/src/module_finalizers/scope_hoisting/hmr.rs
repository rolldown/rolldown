use oxc::{
  ast::{
    NONE,
    ast::{self, PropertyKind},
  },
  span::SPAN,
};
use rolldown_ecmascript_utils::{ExpressionExt, quote_stmt};

use super::ScopeHoistingFinalizer;

impl<'ast> ScopeHoistingFinalizer<'_, 'ast> {
  pub fn generate_hmr_header(&self) -> Vec<ast::Statement<'ast>> {
    let mut ret = vec![];
    if !self.ctx.options.is_hmr_enabled() {
      return ret;
    }

    // `import.meta.hot = __rolldown_runtime__.createModuleHotContext(moduleId);`
    ret.push(self.generate_stmt_of_init_module_hot_context());

    ret.extend(self.generate_runtime_module_register_for_hmr());

    ret
  }

  fn generate_runtime_module_register_for_hmr(&self) -> Vec<ast::Statement<'ast>> {
    let mut ret = vec![];
    if !self.ctx.options.is_hmr_enabled() {
      return ret;
    }

    let module_exports = match self.ctx.module.exports_kind {
      rolldown_common::ExportsKind::Esm => {
        let binding_name_for_namespace_object_ref =
          self.canonical_name_for(self.ctx.module.namespace_object_ref);

        // Add __esModule flag
        ret.push(self.snippet.builder.statement_expression(
          SPAN,
          self.snippet.call_expr_with_arg_expr(
            self.snippet.id_ref_expr("__rolldown_runtime__.__toCommonJS", SPAN),
            self.snippet.id_ref_expr(binding_name_for_namespace_object_ref.as_str(), SPAN),
            false,
          ),
        ));

        // { exports: namespace }
        ast::Argument::ObjectExpression(self.snippet.builder.alloc_object_expression(
          SPAN,
          self.snippet.builder.vec1(self.snippet.builder.object_property_kind_object_property(
            SPAN,
            PropertyKind::Init,
            self.snippet.builder.property_key_static_identifier(SPAN, "exports"),
            self.snippet.id_ref_expr(binding_name_for_namespace_object_ref, SPAN),
            true,
            false,
            false,
          )),
        ))
      }
      rolldown_common::ExportsKind::CommonJs => {
        // `module`
        ast::Argument::Identifier(self.snippet.builder.alloc_identifier_reference(SPAN, "module"))
      }
      rolldown_common::ExportsKind::None => ast::Argument::ObjectExpression(
        // `{}`
        self.snippet.builder.alloc_object_expression(SPAN, self.snippet.builder.vec()),
      ),
    };

    // __rolldown_runtime__.registerModule(moduleId, module)
    let arguments = self.snippet.builder.vec_from_array([
      ast::Argument::StringLiteral(self.snippet.builder.alloc_string_literal(
        SPAN,
        self.snippet.builder.atom(&self.ctx.module.stable_id),
        None,
      )),
      module_exports,
    ]);

    let register_call = self.snippet.builder.alloc_call_expression(
      SPAN,
      self.snippet.id_ref_expr("__rolldown_runtime__.registerModule", SPAN),
      NONE,
      arguments,
      false,
    );

    ret.push(ast::Statement::ExpressionStatement(
      self
        .snippet
        .builder
        .alloc_expression_statement(SPAN, ast::Expression::CallExpression(register_call)),
    ));

    ret
  }

  pub fn generate_stmt_of_init_module_hot_context(&self) -> ast::Statement<'ast> {
    let hot_name = self.canonical_name_for(self.ctx.module.ecma_view.hmr_hot_ref.unwrap());
    // import.meta.hot = __rolldown_runtime__.createModuleHotContext(moduleId);
    quote_stmt(
      self.alloc,
      &format!(
        "const {} = __rolldown_runtime__.createModuleHotContext({:?});",
        hot_name, self.ctx.module.stable_id
      ),
    )
  }

  pub fn rewrite_import_meta_hot(&self, expr: &mut ast::Expression<'ast>) {
    if expr.is_import_meta_hot() {
      if let Some(hmr_hot_ref) = self.ctx.module.ecma_view.hmr_hot_ref {
        let hot_name = self.canonical_name_for(hmr_hot_ref);
        *expr = self.snippet.id_ref_expr(hot_name, SPAN);
      }
    }
  }

  pub fn rewrite_hot_accept_call_deps(&self, call_expr: &mut ast::CallExpression<'ast>) {
    // Check whether the callee is `import.meta.hot.accept`.
    if !call_expr.callee.is_import_meta_hot_accept() {
      return;
    }

    if call_expr.arguments.is_empty() {
      // `import.meta.hot.accept()`
      return;
    }

    match &mut call_expr.arguments[0] {
      ast::Argument::StringLiteral(string_literal) => {
        // `import.meta.hot.accept('./dep.js', ...)`
        let import_record = &self.ctx.module.import_records[self
          .ctx
          .module
          .hmr_info
          .module_request_to_import_record_idx[string_literal.value.as_str()]];
        string_literal.value =
          self.snippet.builder.atom(self.ctx.modules[import_record.resolved_module].stable_id());
      }
      ast::Argument::ArrayExpression(array_expression) => {
        // `import.meta.hot.accept(['./dep1.js', './dep2.js'], ...)`
        array_expression.elements.iter_mut().for_each(|element| {
          if let ast::ArrayExpressionElement::StringLiteral(string_literal) = element {
            let import_record = &self.ctx.module.import_records[self
              .ctx
              .module
              .hmr_info
              .module_request_to_import_record_idx[string_literal.value.as_str()]];
            string_literal.value = self
              .snippet
              .builder
              .atom(self.ctx.modules[import_record.resolved_module].stable_id());
          }
        });
      }
      _ => {}
    }
  }
}
