use oxc::ast::ast::{self, IdentifierName, ImportExpression, StaticMemberExpression};
use oxc::span::Span;
use rolldown_common::AstScopes;

pub trait CallExpressionExt<'ast> {
  fn extract_call_expression_kind(&self, scope: &AstScopes) -> CallExpressionKind;
}

pub enum CallExpressionKind {
  GlobalRequire,
  Other,
  /// Span of ImportExpression
  ImportThen(Span),
}

impl CallExpressionKind {
  /// Returns `true` if the call expression kind is [`GlobalRequire`].
  ///
  /// [`GlobalRequire`]: CallExpressionKind::GlobalRequire
  #[must_use]
  pub fn is_global_require(&self) -> bool {
    matches!(self, Self::GlobalRequire)
  }

  /// Returns `true` if the call expression kind is [`ImportThen`].
  ///
  /// [`ImportThen`]: CallExpressionKind::ImportThen
  #[must_use]
  pub fn is_import_then(&self) -> bool {
    matches!(self, Self::ImportThen(_))
  }
}

impl<'ast> CallExpressionExt<'ast> for ast::CallExpression<'ast> {
  fn extract_call_expression_kind(&self, scope: &AstScopes) -> CallExpressionKind {
    match &self.callee {
      ast::Expression::Identifier(ident) if ident.name == "require" => {
        let Some(ref_id) = ident.reference_id.get() else {
          // `require(...)` inserted by bundler does not have a reference id
          return CallExpressionKind::GlobalRequire;
        };
        if scope.is_unresolved(ref_id) {
          CallExpressionKind::GlobalRequire
        } else {
          CallExpressionKind::Other
        }
      }
      ast::Expression::StaticMemberExpression(member_expr) => {
        if matches!(member_expr.object, ast::Expression::ImportExpression { .. })
          && matches!(&member_expr.property, IdentifierName { name, .. } if name == "then")
        {
          CallExpressionKind::ImportThen(member_expr.span)
        } else {
          CallExpressionKind::Other
        }
      }
      _ => CallExpressionKind::Other,
    }
  }
}
