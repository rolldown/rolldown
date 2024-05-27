use oxc::ast::ast;
use rolldown_common::AstScope;

pub trait CallExpressionExt<'ast> {
  fn is_global_require_call(&self, scope: &AstScope) -> bool;
}

impl<'ast> CallExpressionExt<'ast> for ast::CallExpression<'ast> {
  fn is_global_require_call(&self, scope: &AstScope) -> bool {
    matches!(&self.callee,  ast::Expression::Identifier(ident) if ident.name == "require"
    && scope.is_unresolved(
      ident.reference_id.get().expect("require should have a reference id")))
  }
}
