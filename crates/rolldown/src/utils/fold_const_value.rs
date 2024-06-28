use oxc::allocator::Allocator;
use oxc::ast::{ast::*, visit::walk_mut, AstBuilder, VisitMut};
use oxc::span::SPAN;
use oxc::syntax::operator::BinaryOperator;

/// This is the very basic version of Inline Binary, aiming to support `process.env.NODE_ENV === "production"` -> `true` or `false`.
/// Used before dead branch elimination.
pub struct InlineBinaryValueLight<'a> {
  ast: AstBuilder<'a>,
}

impl<'a> InlineBinaryValueLight<'a> {
  pub fn new(allocator: &'a Allocator) -> Self {
    Self { ast: AstBuilder::new(allocator) }
  }

  pub fn build(&mut self, program: &mut Program<'a>) {
    self.visit_program(program);
  }

  pub fn is_two_expr_equal_in_literal(left: &Expression<'a>, right: &Expression<'a>) -> bool {
    match (left, right) {
      (Expression::BooleanLiteral(left), Expression::BooleanLiteral(right)) => {
        left.value == right.value
      }
      (Expression::StringLiteral(left), Expression::StringLiteral(right)) => {
        left.value == right.value
      }
      (Expression::NumericLiteral(left), Expression::NumericLiteral(right)) => {
        left.value == right.value
      }
      (Expression::BigIntLiteral(left), Expression::BigIntLiteral(right)) => left.raw == right.raw,
      (Expression::NullLiteral(left), Expression::NullLiteral(right)) => true,
      _ => false,
    }
  }
}

impl<'a> VisitMut<'a> for InlineBinaryValueLight<'a> {
  fn visit_expression(&mut self, expr: &mut Expression<'a>) {
    let Expression::BinaryExpression(bin) = expr else {
      return;
    };
    if !matches!(bin.operator, BinaryOperator::Equality | BinaryOperator::StrictEquality) {
      return;
    }
    *expr = Expression::BooleanLiteral(self.ast.alloc(
      self.ast.boolean_literal(SPAN, Self::is_two_expr_equal_in_literal(&bin.left, &bin.right)),
    ));
  }
}
