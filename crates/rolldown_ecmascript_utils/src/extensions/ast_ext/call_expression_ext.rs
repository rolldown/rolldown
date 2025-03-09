use oxc::{ast::ast, semantic::Scoping};
use rolldown_common::AstScopes;

pub trait CallExpressionExt<'ast> {
  fn is_global_require_call(&self, scope: &AstScopes, scoping: &Scoping) -> bool;
}

impl<'ast> CallExpressionExt<'ast> for ast::CallExpression<'ast> {
  fn is_global_require_call(&self, scope: &AstScopes, scoping: &Scoping) -> bool {
    match &self.callee {
      ast::Expression::Identifier(ident) if ident.name == "require" => {
        let Some(ref_id) = ident.reference_id.get() else {
          // `require(...)` inserted by bundler does not have a reference id
          return true;
        };
        scope.is_unresolved(ref_id, scoping)
      }
      _ => false,
    }
  }
}
