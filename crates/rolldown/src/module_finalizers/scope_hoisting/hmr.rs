use oxc::{
  allocator::{Dummy, IntoIn},
  ast::{NONE, ast},
  span::SPAN,
};
use rolldown_ecmascript_utils::quote_stmt;
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
        let mut arg_obj_expr =
          self.snippet.builder.alloc_object_expression(SPAN, self.snippet.builder.vec());

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
    let mut arguments = self.snippet.builder.vec_from_array([
      ast::Argument::StringLiteral(self.snippet.builder.alloc_string_literal(
        SPAN,
        &self.ctx.module.stable_id,
        None,
      )),
      module_exports,
    ]);

    if self.ctx.module.exports_kind.is_commonjs() {
      // __rolldown_runtime__.register(moduleId, module, { cjs: true })
      arguments.push(ast::Argument::ObjectExpression(
        self.snippet.builder.alloc_object_expression(
          SPAN,
          self.snippet.builder.vec1(ast::ObjectPropertyKind::ObjectProperty(
            ast::ObjectProperty {
              key: ast::PropertyKey::StaticIdentifier(
                self.snippet.id_name("cjs", SPAN).into_in(self.alloc),
              ),
              value: ast::Expression::BooleanLiteral(
                self.snippet.builder.alloc_boolean_literal(SPAN, true),
              ),
              ..ast::ObjectProperty::dummy(self.alloc)
            }
            .into_in(self.alloc),
          )),
        ),
      ));
    }

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
    // import.meta.hot = __rolldown_runtime__.createModuleHotContext(moduleId);
    let stmt = quote_stmt(
      self.alloc,
      &format!(
        "import.meta.hot = __rolldown_runtime__.createModuleHotContext({:?});",
        self.ctx.module.stable_id
      ),
    );
    stmt
  }
}
