use oxc::{
  ast::ast::{self, Expression},
  span::SPAN,
};
use rolldown_ecmascript_utils::{
  ExpressionExt, ExpressionFactoryExt as _, MemberExpressionFactoryExt as _,
};

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

  /// `if (__rolldown_runtime__.hasFactory(id)) return module.exports = __rolldown_runtime__.initModule(id);`
  ///
  /// Heads the body of a CommonJS wrapper in the entry chunk. The body runs at the first
  /// `require_x()`, which can come after the file was edited; a patch may have registered
  /// the newer code as a factory by then, and the old body must not run instead.
  pub fn generate_cjs_wrapper_factory_dispatch_stmt(&self) -> ast::Statement<'ast> {
    let runtime_call = |method: &str| {
      ast::Expression::new_call_expression(
        SPAN,
        Expression::new_id_ref_expr(SPAN, method, self),
        None,
        oxc::allocator::Vec::from_iter_in([self.module_id_argument()], self),
        false,
        self,
      )
    };
    let assign_exports = ast::Expression::new_assignment_expression(
      SPAN,
      ast::AssignmentOperator::Assign,
      ast::AssignmentTarget::from(ast::SimpleAssignmentTarget::from(
        ast::MemberExpression::new_member_access("module", "exports", self),
      )),
      runtime_call("__rolldown_runtime__.initModule"),
      self,
    );
    ast::Statement::new_if_statement(
      SPAN,
      runtime_call("__rolldown_runtime__.hasFactory"),
      ast::Statement::new_return_statement(SPAN, Some(assign_exports), self),
      None,
      self,
    )
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
