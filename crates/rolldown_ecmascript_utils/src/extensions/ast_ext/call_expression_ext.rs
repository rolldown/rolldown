use oxc::ast::ast;
use rolldown_common::AstScopes;

pub trait CallExpressionExt<'ast> {
  fn is_global_require_call(&self, scope: &AstScopes) -> bool;
}

impl<'ast> CallExpressionExt<'ast> for ast::CallExpression<'ast> {
  fn is_global_require_call(&self, scope: &AstScopes) -> bool {
    match &self.callee {
      ast::Expression::Identifier(ident) if ident.name == "require" => {
        // `require(...)` inserted by bundler does not have a reference id
        ident.reference_id.get().is_none_or(|ref_id| scope.is_unresolved(ref_id))
      }
      _ => false,
    }
  }
}
