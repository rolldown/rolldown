use oxc::{ast::ast, semantic::SymbolTable};
use rolldown_common::AstScopes;

pub trait CallExpressionExt<'ast> {
  fn is_global_require_call(&self, scope: &AstScopes, symbol_table: &SymbolTable) -> bool;
}

impl<'ast> CallExpressionExt<'ast> for ast::CallExpression<'ast> {
  fn is_global_require_call(&self, scope: &AstScopes, symbol_table: &SymbolTable) -> bool {
    match &self.callee {
      ast::Expression::Identifier(ident) if ident.name == "require" => {
        let Some(ref_id) = ident.reference_id.get() else {
          // `require(...)` inserted by bundler does not have a reference id
          return true;
        };
        scope.is_unresolved(ref_id, symbol_table)
      }
      _ => false,
    }
  }
}
