use oxc::{
  allocator::{Dummy, IntoIn},
  ast::{NONE, ast},
  span::SPAN,
};
use rolldown_ecmascript_utils::{ExpressionExt, quote_stmt};
use rolldown_utils::ecmascript::is_validate_identifier_name;

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
        // TODO: Still we could reuse use module namespace def

        // Empty object `{}`
        let mut arg_obj_expr = self.snippet.builder.alloc_object_expression(
          SPAN,
          self.snippet.builder.vec_with_capacity(
            self.ctx.linking_info.canonical_exports_len() + 1, /* __esModule */
          ),
        );

        self.ctx.linking_info.canonical_exports().for_each(|(export, resolved_export)| {
          // prop_name: () => returned
          let prop_name = export;
          let returned =
            self.finalized_expr_for_symbol_ref(resolved_export.symbol_ref, false, None);
          arg_obj_expr.properties.push(ast::ObjectPropertyKind::ObjectProperty(
            ast::ObjectProperty {
              key: if is_validate_identifier_name(prop_name) {
                ast::PropertyKey::StaticIdentifier(
                  self.snippet.id_name(prop_name, SPAN).into_in(self.alloc),
                )
              } else {
                ast::PropertyKey::StringLiteral(self.snippet.alloc_string_literal(prop_name, SPAN))
              },
              value: self.snippet.only_return_arrow_expr(returned),
              ..ast::ObjectProperty::dummy(self.alloc)
            }
            .into_in(self.alloc),
          ));
        });
        // Add __esModule flag
        arg_obj_expr.properties.push(
          self
            .snippet
            .object_property_kind_object_property(
              "__esModule",
              self.snippet.builder.expression_boolean_literal(SPAN, true),
              false,
            )
            .into_in(self.alloc),
        );
        ast::Argument::ObjectExpression(arg_obj_expr)
      }
      rolldown_common::ExportsKind::CommonJs => {
        // `module.exports`
        ast::Argument::StaticMemberExpression(self.snippet.builder.alloc_static_member_expression(
          SPAN,
          self.snippet.id_ref_expr("module", SPAN),
          self.snippet.id_name("exports", SPAN),
          false,
        ))
      }
      rolldown_common::ExportsKind::None => ast::Argument::ObjectExpression(
        // `{}`
        self.snippet.builder.alloc_object_expression(SPAN, self.snippet.builder.vec()),
      ),
    };

    // __rolldown_runtime__.register(moduleId, module)
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
    let stmt = quote_stmt(
      self.alloc,
      &format!(
        "const {} = __rolldown_runtime__.createModuleHotContext({:?});",
        hot_name, self.ctx.module.stable_id
      ),
    );
    stmt
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
