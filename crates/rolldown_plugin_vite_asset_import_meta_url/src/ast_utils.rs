use std::borrow::Cow;

use oxc::ast::ast::{
  Argument, BinaryExpression, BinaryOperator, CallExpression, Expression, TemplateLiteral,
};

pub fn template_literal_to_glob<'a>(node: &TemplateLiteral) -> anyhow::Result<Cow<'a, str>> {
  let mut glob = String::new();
  for (index, quasi) in node.quasis.iter().enumerate() {
    glob += &sanitize_string(&quasi.value.raw)?;
    if let Some(expr) = node.expressions.get(index) {
      glob += &expr_to_glob(expr)?;
    }
  }
  Ok(Cow::Owned(glob))
}

fn expr_to_glob<'a>(expr: &'a Expression) -> anyhow::Result<Cow<'a, str>> {
  match expr {
    Expression::TemplateLiteral(node) => template_literal_to_glob(node),
    Expression::CallExpression(node) => call_expr_to_glob(node),
    Expression::BinaryExpression(node) => binary_expr_to_glob(node),
    Expression::StringLiteral(node) => sanitize_string(&node.value),
    _ => Ok(Cow::Borrowed("*")),
  }
}

fn sanitize_string(s: &str) -> anyhow::Result<Cow<'_, str>> {
  if s.contains('*') {
    Err(anyhow::anyhow!("A dynamic import cannot contain * characters."))?;
  }
  Ok(Cow::Borrowed(s))
}

fn call_expr_to_glob<'a>(node: &'a CallExpression) -> anyhow::Result<Cow<'a, str>> {
  if let Expression::StaticMemberExpression(member_expr) = &node.callee {
    if member_expr.property.name == "concat" {
      if node.arguments.is_empty() {
        return expr_to_glob(&member_expr.object);
      }
      let mut glob = expr_to_glob(&member_expr.object)?.into_owned();
      for arg in &node.arguments {
        let part = match arg {
          Argument::SpreadElement(_) => "*",
          _ => &expr_to_glob(arg.to_expression())?,
        };
        glob += part;
      }
      return Ok(Cow::Owned(glob));
    }
  }
  Ok(Cow::Borrowed("*"))
}

fn binary_expr_to_glob<'a>(node: &'a BinaryExpression) -> anyhow::Result<Cow<'a, str>> {
  if node.operator != BinaryOperator::Addition {
    return Err(anyhow::anyhow!("{:?} operator is not supported.", node.operator.as_str()));
  }
  let left = expr_to_glob(&node.left)?;
  let right = expr_to_glob(&node.right)?;
  Ok(Cow::Owned(rolldown_utils::concat_string!(left, right)))
}
