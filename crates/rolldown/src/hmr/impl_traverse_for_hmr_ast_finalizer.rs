use oxc::ast::ast;
use oxc_traverse::Traverse;
use rolldown_ecmascript_utils::quote_expr;

use crate::hmr::hmr_ast_finalizer::HmrAstFinalizer;

impl<'ast> Traverse<'ast, ()> for HmrAstFinalizer<'_, 'ast> {
  fn exit_expression(
    &mut self,
    node: &mut oxc::ast::ast::Expression<'ast>,
    ctx: &mut oxc_traverse::TraverseCtx<'ast, ()>,
  ) {
    if ctx.is_current_scope_valid_for_tla() && matches!(node, ast::Expression::ThisExpression(_)) {
      // Rewrite this to `undefined` or `exports`
      if self.module.exports_kind.is_commonjs() {
        // Rewrite this to `exports`
        *node = quote_expr(self.alloc, "exports");
      } else {
        // Rewrite this to `undefined`
        *node = quote_expr(self.alloc, "void 0");
      }
    }
  }
}

trait TraverseCtxExt<'ast> {
  fn is_current_scope_valid_for_tla(&self) -> bool;
}

impl<'ast> TraverseCtxExt<'ast> for oxc_traverse::TraverseCtx<'ast, ()> {
  fn is_current_scope_valid_for_tla(&self) -> bool {
    // self.scope_stack.iter().rev().all(|flag| flag.is_block() || flag.is_top())
    let scoping = self.scoping();
    scoping
      .scope_ancestors(self.current_scope_id())
      .map(|scope_id| scoping.scope_flags(scope_id))
      .all(|scope_flags| scope_flags.is_block() || scope_flags.is_top())
  }
}
