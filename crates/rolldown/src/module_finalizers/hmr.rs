use oxc::{
  ast::ast::{self, Expression},
  span::SPAN,
};
use rolldown_ecmascript_utils::{ExpressionExt, ExpressionFactoryExt as _};

use crate::hmr::utils::HmrAstBuilder;

use super::ScopeHoistingFinalizer;

impl<'ast> ScopeHoistingFinalizer<'_, 'ast> {
  pub fn generate_hmr_header(&self) -> Vec<ast::Statement<'ast>> {
    let mut ret = vec![];
    if !self.ctx.options.is_dev_mode_enabled() {
      return ret;
    }

    // `var $hot = __rolldown_runtime__.createModuleHotContext(moduleId);`
    ret.push(self.create_module_hot_context_initializer_stmt());

    ret.push(self.create_register_module_stmt());

    ret
  }

  pub fn rewrite_import_meta_hot(&self, expr: &mut ast::Expression<'ast>) {
    if expr.is_import_meta_hot() {
      if let Some(hmr_hot_ref) = self.ctx.module.ecma_view.hmr_hot_ref {
        let hot_name = self.canonical_name_for(hmr_hot_ref);
        *expr = Expression::new_id_ref_expr(SPAN, hot_name, self);
      }
    }
  }

  pub fn rewrite_hot_accept_call_deps(&self, call_expr: &mut ast::CallExpression<'ast>) {
    if !self.ctx.options.is_dev_mode_enabled() {
      return;
    }
    crate::hmr::utils::rewrite_hot_accept_deps(
      call_expr,
      self.ctx.module,
      self.ctx.modules,
      &self.ast_builder,
    );
  }
}
