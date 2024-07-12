use oxc::{
  ast::{ast::Expression, Comment, CommentKind},
  span::Span,
  syntax::operator::{BinaryOperator, LogicalOperator, UnaryOperator, UpdateOperator},
};
use rolldown_common::AstScopes;

use super::SideEffectDetector;

impl<'a> SideEffectDetector<'a> {
  /// Get the nearest comment before the `span`, return `None` if no leading comment is founded.
  ///
  ///  # Examples
  /// ```javascript
  /// /* valid comment for `a`  */ let a = 1;
  ///
  /// // valid comment for `b`
  /// let b = 1;
  ///
  /// // valid comment for `c`
  ///
  ///
  /// let c = 1;
  ///
  /// let d = 1; /* valid comment for `e` */
  /// let e = 2
  /// ```
  /// Derived from https://github.com/oxc-project/oxc/blob/147864cfeb112df526bb83d5b8671b465c005066/crates/oxc_linter/src/utils/tree_shaking.rs#L204
  pub fn leading_comment_for(&self, span: Span) -> Option<(&Comment, &str)> {
    let comment = self.trivias.comments_range(..span.start).next_back()?;

    let comment_text = comment.span.source_text(self.source);
    // If there are non-whitespace characters between the `comment` and the `span`,
    // we treat the `comment` not belongs to the `span`.
    let range_text = Span::new(comment.span.end, span.start).source_text(self.source);
    let only_whitespace = match range_text.strip_prefix("*/") {
      Some(str) => str.trim().is_empty(),
      None => range_text.trim().is_empty(),
    };
    if !only_whitespace {
      return None;
    }

    Some((comment, comment_text))
  }
}

#[derive(PartialEq, Eq, Copy, Clone)]
pub(crate) enum PrimitiveType {
  Null,
  Undefined,
  Boolean,
  Number,
  String,
  BigInt,
  Mixed,
  Unknown,
}

fn merged_known_primitive_types(
  scope: &AstScopes,
  left: &Expression,
  right: &Expression,
) -> PrimitiveType {
  let left_type = known_primitive_type(scope, left);
  if left_type == PrimitiveType::Unknown {
    return PrimitiveType::Unknown;
  }
  let right_type = known_primitive_type(scope, right);
  if right_type == PrimitiveType::Unknown {
    return PrimitiveType::Unknown;
  }
  if right_type == left_type {
    return right_type;
  }
  PrimitiveType::Mixed
}

#[allow(clippy::too_many_lines)]
pub(crate) fn known_primitive_type(scope: &AstScopes, expr: &Expression) -> PrimitiveType {
  match expr {
    Expression::NullLiteral(_) => PrimitiveType::Null,
    Expression::Identifier(id)
      if id.name == "undefined" && scope.is_unresolved(id.reference_id.get().unwrap()) =>
    {
      PrimitiveType::Undefined
    }
    Expression::BooleanLiteral(_) => PrimitiveType::Boolean,
    Expression::NumericLiteral(_) => PrimitiveType::Number,
    Expression::StringLiteral(_) => PrimitiveType::String,
    Expression::BigIntLiteral(_) => PrimitiveType::BigInt,
    Expression::TemplateLiteral(e) => {
      if e.expressions.is_empty() {
        PrimitiveType::String
      } else {
        PrimitiveType::Unknown
      }
    }
    Expression::UpdateExpression(e) => {
      match e.operator {
        UpdateOperator::Increment | UpdateOperator::Decrement => {
          PrimitiveType::Mixed // Can be number or bigint
        }
      }
    }
    Expression::UnaryExpression(e) => match e.operator {
      UnaryOperator::Void => PrimitiveType::Undefined,
      UnaryOperator::Typeof => PrimitiveType::String,
      UnaryOperator::LogicalNot | UnaryOperator::Delete => PrimitiveType::Boolean,
      UnaryOperator::UnaryPlus => PrimitiveType::Number, // Cannot be bigint because that throws an exception
      UnaryOperator::UnaryNegation | UnaryOperator::BitwiseNot => {
        let value = known_primitive_type(scope, &e.argument);
        if value == PrimitiveType::BigInt {
          return PrimitiveType::BigInt;
        }
        if value != PrimitiveType::Unknown && value != PrimitiveType::Mixed {
          return PrimitiveType::Number;
        }
        PrimitiveType::Mixed // Can be number or bigint
      }
    },
    Expression::LogicalExpression(e) => match e.operator {
      LogicalOperator::Or | LogicalOperator::And => {
        merged_known_primitive_types(scope, &e.left, &e.right)
      }
      LogicalOperator::Coalesce => {
        let left = known_primitive_type(scope, &e.left);
        let right = known_primitive_type(scope, &e.right);
        if left == PrimitiveType::Null || left == PrimitiveType::Undefined {
          return right;
        }
        if left != PrimitiveType::Unknown {
          if left != PrimitiveType::Mixed {
            return left; // Definitely not null or undefined
          }
          if right != PrimitiveType::Unknown {
            return PrimitiveType::Mixed; // Definitely some kind of primitive
          }
        }
        PrimitiveType::Unknown
      }
    },
    Expression::BinaryExpression(e) => match e.operator {
      BinaryOperator::StrictEquality
      | BinaryOperator::StrictInequality
      | BinaryOperator::Equality
      | BinaryOperator::Inequality
      | BinaryOperator::LessThan
      | BinaryOperator::GreaterThan
      | BinaryOperator::GreaterEqualThan
      | BinaryOperator::LessEqualThan
      | BinaryOperator::Instanceof
      | BinaryOperator::In => PrimitiveType::Boolean,
      BinaryOperator::Addition => {
        let left = known_primitive_type(scope, &e.left);
        let right = known_primitive_type(scope, &e.right);
        if left == PrimitiveType::String || right == PrimitiveType::String {
          PrimitiveType::String
        } else if left == PrimitiveType::BigInt && right == PrimitiveType::BigInt {
          PrimitiveType::BigInt
        } else if !matches!(
          left,
          PrimitiveType::Unknown | PrimitiveType::Mixed | PrimitiveType::BigInt
        ) && !matches!(
          right,
          PrimitiveType::Unknown | PrimitiveType::Mixed | PrimitiveType::BigInt
        ) {
          PrimitiveType::Number
        } else {
          PrimitiveType::Mixed // Can be number or bigint or string (or an exception)
        }
      }
      BinaryOperator::Subtraction
      | BinaryOperator::Multiplication
      | BinaryOperator::Division
      | BinaryOperator::Remainder
      | BinaryOperator::Exponential
      | BinaryOperator::BitwiseAnd
      | BinaryOperator::BitwiseOR
      | BinaryOperator::ShiftRight
      | BinaryOperator::ShiftLeft
      | BinaryOperator::ShiftRightZeroFill
      | BinaryOperator::BitwiseXOR => PrimitiveType::Mixed,
    },

    Expression::AssignmentExpression(e) => match e.operator {
      oxc::syntax::operator::AssignmentOperator::Assign => known_primitive_type(scope, &e.right),
      oxc::syntax::operator::AssignmentOperator::Addition => {
        let right = known_primitive_type(scope, &e.right);
        if right == PrimitiveType::String {
          PrimitiveType::String
        } else {
          PrimitiveType::Mixed // Can be number or bigint or string (or an exception)
        }
      }
      oxc::syntax::operator::AssignmentOperator::Subtraction
      | oxc::syntax::operator::AssignmentOperator::Multiplication
      | oxc::syntax::operator::AssignmentOperator::Division
      | oxc::syntax::operator::AssignmentOperator::Remainder
      | oxc::syntax::operator::AssignmentOperator::ShiftLeft
      | oxc::syntax::operator::AssignmentOperator::ShiftRight
      | oxc::syntax::operator::AssignmentOperator::ShiftRightZeroFill
      | oxc::syntax::operator::AssignmentOperator::BitwiseOR
      | oxc::syntax::operator::AssignmentOperator::BitwiseXOR
      | oxc::syntax::operator::AssignmentOperator::BitwiseAnd
      | oxc::syntax::operator::AssignmentOperator::LogicalAnd
      | oxc::syntax::operator::AssignmentOperator::LogicalOr
      | oxc::syntax::operator::AssignmentOperator::LogicalNullish
      | oxc::syntax::operator::AssignmentOperator::Exponential => PrimitiveType::Mixed,
    },
    _ => PrimitiveType::Unknown,
  }
}
