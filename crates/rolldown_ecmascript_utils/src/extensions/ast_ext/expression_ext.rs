use oxc::{ast::ast, span::Atom};

pub trait ExpressionExt<'ast> {
  fn as_call_expression(&self) -> Option<&ast::CallExpression<'ast>>;
  fn as_call_expression_mut(&mut self) -> Option<&mut ast::CallExpression<'ast>>;

  fn as_identifier(&self) -> Option<&ast::IdentifierReference<'ast>>;
  fn as_identifier_mut(&mut self) -> Option<&mut ast::IdentifierReference<'ast>>;
  fn as_unary_expression(&self) -> Option<&ast::UnaryExpression<'ast>>;
  fn as_string_literal(&self) -> Option<&ast::StringLiteral<'ast>>;
  fn as_binary_expression(&self) -> Option<&ast::BinaryExpression<'ast>>;
  fn as_static_member_expr_mut(&mut self) -> Option<&mut ast::StaticMemberExpression<'ast>>;
  fn as_static_module_request(&self) -> Option<Atom<'ast>>;

  fn is_import_meta(&self) -> bool;
  fn is_import_meta_url(&self) -> bool;
  fn is_import_meta_hot(&self) -> bool;
  fn is_import_meta_hot_accept(&self) -> bool;
}

impl<'ast> ExpressionExt<'ast> for ast::Expression<'ast> {
  fn as_call_expression(&self) -> Option<&ast::CallExpression<'ast>> {
    if let ast::Expression::CallExpression(call_expr) = self { Some(call_expr) } else { None }
  }

  fn as_call_expression_mut(&mut self) -> Option<&mut ast::CallExpression<'ast>> {
    if let ast::Expression::CallExpression(call_expr) = self { Some(call_expr) } else { None }
  }

  fn as_identifier(&self) -> Option<&ast::IdentifierReference<'ast>> {
    if let ast::Expression::Identifier(ident) = self { Some(ident) } else { None }
  }

  fn as_identifier_mut(&mut self) -> Option<&mut ast::IdentifierReference<'ast>> {
    if let ast::Expression::Identifier(ident) = self { Some(ident) } else { None }
  }

  fn as_unary_expression(&self) -> Option<&ast::UnaryExpression<'ast>> {
    let ast::Expression::UnaryExpression(expr) = self else {
      return None;
    };
    Some(expr)
  }

  fn as_string_literal(&self) -> Option<&ast::StringLiteral<'ast>> {
    let ast::Expression::StringLiteral(expr) = self else {
      return None;
    };
    Some(expr)
  }

  fn as_static_module_request(&self) -> Option<Atom<'ast>> {
    match &self {
      ast::Expression::StringLiteral(request) => Some(request.value),
      ast::Expression::TemplateLiteral(request) if request.is_no_substitution_template() => {
        request.quasi()
      }
      _ => None,
    }
  }

  fn as_binary_expression(&self) -> Option<&ast::BinaryExpression<'ast>> {
    let ast::Expression::BinaryExpression(expr) = self else {
      return None;
    };
    Some(expr)
  }

  /// // Check if the expression is `import.meta`
  fn is_import_meta(&self) -> bool {
    matches!(self, ast::Expression::MetaProperty(meta_prop)
    if meta_prop.meta.name == "import" && meta_prop.property.name == "meta")
  }

  /// Check if the expression is `import.meta.url`
  fn is_import_meta_url(&self) -> bool {
    matches!(self, ast::Expression::StaticMemberExpression(member_expr)
    if member_expr.object.is_import_meta() && member_expr.property.name == "url")
  }

  /// Check if the expression is `import.meta.hot`
  fn is_import_meta_hot(&self) -> bool {
    matches!(self, ast::Expression::StaticMemberExpression(member_expr)
    if member_expr.object.is_import_meta() && member_expr.property.name == "hot")
  }

  /// Check if the expression is `import.meta.hot.accept`
  fn is_import_meta_hot_accept(&self) -> bool {
    matches!(self, ast::Expression::StaticMemberExpression(member_expr)
    if member_expr.object.is_import_meta_hot() && member_expr.property.name == "accept")
  }

  fn as_static_member_expr_mut(&mut self) -> Option<&mut ast::StaticMemberExpression<'ast>> {
    if let ast::Expression::StaticMemberExpression(member_expr) = self {
      Some(member_expr)
    } else {
      None
    }
  }
}
