use oxc::allocator::Box;
use oxc::ast::ast::{
  Expression, JSXMemberExpression, JSXMemberExpressionObject, StaticMemberExpression,
};

pub trait JsxExt<'ast> {
  type AstKind;
  fn rewrite_ident_reference(&mut self, ident_ref: JSXMemberExpressionObject<'ast>);
  fn from_ast(
    member_expr: Self::AstKind,
    allocator: &'ast oxc::allocator::Allocator,
  ) -> Option<Self>
  where
    Self: Sized;
}

impl<'ast> JsxExt<'ast> for JSXMemberExpressionObject<'ast> {
  type AstKind = Expression<'ast>;
  fn rewrite_ident_reference(&mut self, ident_ref: JSXMemberExpressionObject<'ast>) {
    let mut object = self;
    loop {
      match object {
        JSXMemberExpressionObject::IdentifierReference(_ident) => {
          *object = ident_ref;
          break;
        }
        JSXMemberExpressionObject::MemberExpression(member_expr) => {
          object = &mut member_expr.object;
        }
        JSXMemberExpressionObject::ThisExpression(_) => break,
      }
    }
  }

  fn from_ast(
    member_expr: Expression<'ast>,
    allocator: &'ast oxc::allocator::Allocator,
  ) -> Option<Self> {
    match member_expr {
      Expression::Identifier(ident) => Some(JSXMemberExpressionObject::IdentifierReference(ident)),
      Expression::StaticMemberExpression(member_expr) => {
        Some(JSXMemberExpressionObject::MemberExpression(Box::new_in(
          JSXMemberExpression::from_ast(member_expr.unbox(), allocator)?,
          allocator,
        )))
      }
      Expression::ThisExpression(expr) => Some(JSXMemberExpressionObject::ThisExpression(expr)),
      _ => None,
    }
  }
}

impl<'ast> JsxExt<'ast> for JSXMemberExpression<'ast> {
  type AstKind = StaticMemberExpression<'ast>;
  fn rewrite_ident_reference(&mut self, _ident_ref: JSXMemberExpressionObject<'ast>) {
    todo!()
  }

  fn from_ast(
    member_expr: StaticMemberExpression<'ast>,
    allocator: &'ast oxc::allocator::Allocator,
  ) -> Option<Self>
  where
    Self: Sized,
  {
    Some(JSXMemberExpression {
      span: member_expr.span,
      object: JSXMemberExpressionObject::from_ast(member_expr.object, allocator)?,
      property: oxc::ast::ast::JSXIdentifier {
        span: member_expr.span,
        name: member_expr.property.name,
      },
    })
  }
}
