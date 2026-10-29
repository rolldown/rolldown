use std::borrow::Cow;

use oxc::ast::ast;
use rolldown_utils::ecmascript::to_usv_str;

pub trait ExpressionExt<'ast> {
  fn as_call_expression(&self) -> Option<&ast::CallExpression<'ast>>;

  fn as_identifier(&self) -> Option<&ast::IdentifierReference<'ast>>;
  fn as_identifier_mut(&mut self) -> Option<&mut ast::IdentifierReference<'ast>>;
  fn as_string_literal(&self) -> Option<&ast::StringLiteral<'ast>>;
  fn as_static_module_request(&self) -> Option<Cow<'ast, str>>;

  fn is_import_meta(&self) -> bool;
  fn is_import_meta_url(&self) -> bool;
  fn is_import_meta_hot(&self) -> bool;
  fn is_import_meta_hot_accept(&self) -> bool;
  fn is_import_meta_hot_accept_exports(&self) -> bool;
}

impl<'ast> ExpressionExt<'ast> for ast::Expression<'ast> {
  fn as_call_expression(&self) -> Option<&ast::CallExpression<'ast>> {
    if let ast::Expression::CallExpression(call_expr) = self { Some(call_expr) } else { None }
  }

  fn as_identifier(&self) -> Option<&ast::IdentifierReference<'ast>> {
    if let ast::Expression::Identifier(ident) = self { Some(ident) } else { None }
  }

  fn as_identifier_mut(&mut self) -> Option<&mut ast::IdentifierReference<'ast>> {
    if let ast::Expression::Identifier(ident) = self { Some(ident) } else { None }
  }

  fn as_string_literal(&self) -> Option<&ast::StringLiteral<'ast>> {
    let ast::Expression::StringLiteral(expr) = self else {
      return None;
    };
    Some(expr)
  }

  /// A lone surrogate becomes U+FFFD, as it does when the runtime resolves the request.
  fn as_static_module_request(&self) -> Option<Cow<'ast, str>> {
    match &self {
      ast::Expression::StringLiteral(request) => Some(to_usv_str(request.value)),
      ast::Expression::TemplateLiteral(request) => request.single_quasi().map(to_usv_str),
      _ => None,
    }
  }

  /// // Check if the expression is `import.meta`
  fn is_import_meta(&self) -> bool {
    matches!(self, ast::Expression::ImportMeta(_))
  }

  /// Check if the expression is `import.meta.url`
  fn is_import_meta_url(&self) -> bool {
    matches!(self, ast::Expression::StaticMemberExpression(member_expr)
    if member_expr.property.name == "url" && member_expr.object.is_import_meta())
  }

  /// Check if the expression is `import.meta.hot`
  fn is_import_meta_hot(&self) -> bool {
    matches!(self, ast::Expression::StaticMemberExpression(member_expr)
    if member_expr.property.name == "hot" && member_expr.object.is_import_meta())
  }

  /// Check if the expression is `import.meta.hot.accept`
  fn is_import_meta_hot_accept(&self) -> bool {
    matches!(self, ast::Expression::StaticMemberExpression(member_expr)
    if member_expr.property.name == "accept" && member_expr.object.is_import_meta_hot())
  }

  /// Check if the expression is `import.meta.hot.acceptExports`
  fn is_import_meta_hot_accept_exports(&self) -> bool {
    matches!(self, ast::Expression::StaticMemberExpression(member_expr)
    if member_expr.property.name == "acceptExports" && member_expr.object.is_import_meta_hot())
  }
}
