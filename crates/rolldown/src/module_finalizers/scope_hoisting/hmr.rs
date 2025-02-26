use oxc::{
  ast::{NONE, ast},
  span::SPAN,
};

use super::ScopeHoistingFinalizer;

impl<'ast> ScopeHoistingFinalizer<'_, 'ast> {
  pub fn generate_runtime_module_register_for_hmr(&self) -> Vec<ast::Statement<'ast>> {
    let mut ret = vec![];
    if !self.ctx.options.is_hmr_enabled() {
      return ret;
    }

    let module_exports = match self.ctx.module.exports_kind {
      rolldown_common::ExportsKind::Esm => {
        // TODO: use namespace
        ast::Argument::ObjectExpression(self.snippet.builder.alloc_object_expression(
          SPAN,
          self.snippet.builder.vec(),
          None,
        ))
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
        self.snippet.builder.alloc_object_expression(SPAN, self.snippet.builder.vec(), None),
      ),
    };

    // __rolldown_runtime__.register(moduleId, module)
    let arguments = self.snippet.builder.vec_from_array([
      ast::Argument::StringLiteral(self.snippet.builder.alloc_string_literal(
        SPAN,
        &self.ctx.module.stable_id,
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
}
