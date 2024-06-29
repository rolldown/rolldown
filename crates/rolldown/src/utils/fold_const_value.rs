use oxc::allocator::Allocator;
use oxc::ast::ast::{Expression, Program};
use oxc::ast::{AstBuilder, VisitMut};
use oxc::span::SPAN;
use oxc::syntax::operator::BinaryOperator;

/// This is a very basic version of binary expression folder, aiming to support converting `process.env.NODE_ENV === "production"` -> `true` or `false`.
/// Used before dead branch elimination.
pub struct BasicInlineBinaryValue<'a> {
  ast: AstBuilder<'a>,
}

impl<'a> BasicInlineBinaryValue<'a> {
  pub fn new(allocator: &'a Allocator) -> Self {
    Self { ast: AstBuilder::new(allocator) }
  }

  pub fn build(&mut self, program: &mut Program<'a>) {
    self.visit_program(program);
  }

  pub fn is_two_expr_equal_in_literal(
    left: &Expression<'a>,
    right: &Expression<'a>,
  ) -> Option<bool> {
    match (left, right) {
      (Expression::BooleanLiteral(left), Expression::BooleanLiteral(right)) => {
        Some(left.value == right.value)
      }
      (Expression::StringLiteral(left), Expression::StringLiteral(right)) => {
        Some(left.value == right.value)
      }
      (Expression::NumericLiteral(left), Expression::NumericLiteral(right)) => {
        // cargo clippy recommends use this method to compare float number instead of `==`
        Some((left.value - right.value).abs() < f64::EPSILON)
      }
      (Expression::BigIntLiteral(left), Expression::BigIntLiteral(right)) => {
        Some(left.raw == right.raw)
      }
      (Expression::NullLiteral(_), Expression::NullLiteral(_)) => Some(true),
      _ => None,
    }
  }
}

impl<'a> VisitMut<'a> for BasicInlineBinaryValue<'a> {
  fn visit_expression(&mut self, expr: &mut Expression<'a>) {
    let Expression::BinaryExpression(bin) = expr else {
      return;
    };
    if !matches!(bin.operator, BinaryOperator::Equality | BinaryOperator::StrictEquality) {
      return;
    }
    if let Some(v) = Self::is_two_expr_equal_in_literal(&bin.left, &bin.right) {
      *expr = Expression::BooleanLiteral(self.ast.alloc(self.ast.boolean_literal(SPAN, v)));
    }
  }
}
