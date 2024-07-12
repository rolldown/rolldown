use oxc::{
  ast::{ast::Expression, Comment, CommentKind},
  span::Span,
  syntax::{
    module_record::ExportExportName,
    operator::{BinaryOperator, LogicalOperator, UnaryOperator, UpdateOperator},
  },
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
  /// let d = 1; /* invalid comment for `e` */
  /// let e = 2
  /// ```
  /// Derived from https://github.com/oxc-project/oxc/blob/147864cfeb112df526bb83d5b8671b465c005066/crates/oxc_linter/src/utils/tree_shaking.rs#L204
  pub fn leading_comment_for(&self, span: Span) -> Option<(&Comment, &str)> {
    let comment = self.trivias.comments_range(..span.start).next_back()?;

    let comment_text = comment.span.source_text(self.source);

    // If there are non-whitespace characters between the `comment`` and the `span`,
    // we treat the `comment` not belongs to the `span`.
    let only_whitespace = Span::new(comment.span.end, span.start)
      .source_text(self.source)
      .strip_prefix("*/") // for multi-line comment
      .is_some_and(|s| s.trim().is_empty());

    if !only_whitespace {
      return None;
    }

    // Next step, we need make sure it's not the trailing comment of the previous line.
    let mut current_line_start = span.start as usize;
    for c in self.source[..span.start as usize].chars().rev() {
      if c == '\n' {
        break;
      }

      current_line_start -= c.len_utf8();
    }
    let Ok(current_line_start) = u32::try_from(current_line_start) else {
      return None;
    };

    if comment.span.end < current_line_start {
      let previous_line =
        self.source[..comment.span.end as usize].lines().next_back().unwrap_or("");
      let nothing_before_comment = previous_line
        .trim()
        .strip_prefix(if comment.kind == CommentKind::SingleLine { "//" } else { "/*" })
        .is_some_and(|s| s.trim().is_empty());
      if !nothing_before_comment {
        return None;
      }
    }

    Some((comment, comment_text))
  }
}

#[derive(PartialEq, Eq, Copy, Clone)]
pub(crate) enum PrimitiveType {
  PrimitiveNull,
  PrimitiveUndefined,
  PrimitiveBoolean,
  PrimitiveNumber,
  PrimitiveString,
  PrimitiveBigInt,
  PrimitiveMixed,
  PrimitiveUnknown,
}

fn merged_known_primitive_types(
  scope: &AstScopes,
  left: &Expression,
  right: &Expression,
) -> PrimitiveType {
  let left_type = known_primitive_type(scope, left);
  if left_type == PrimitiveType::PrimitiveUnknown {
    return PrimitiveType::PrimitiveUnknown;
  }
  let right_type = known_primitive_type(scope, left);
  if right_type == PrimitiveType::PrimitiveUnknown {
    return PrimitiveType::PrimitiveUnknown;
  }
  if right_type == left_type {
    return right_type;
  }
  PrimitiveType::PrimitiveMixed
}

pub(crate) fn known_primitive_type(scope: &AstScopes, expr: &Expression) -> PrimitiveType {
  match expr {
    Expression::NullLiteral(_) => PrimitiveType::PrimitiveNull,
    Expression::Identifier(id) if scope.is_unresolved(id.reference_id.get().unwrap()) => {
      PrimitiveType::PrimitiveUndefined
    }
    Expression::BooleanLiteral(b) => PrimitiveType::PrimitiveBoolean,
    Expression::NumericLiteral(_) => PrimitiveType::PrimitiveNumber,
    Expression::StringLiteral(_) => PrimitiveType::PrimitiveString,
    Expression::BigIntLiteral(_) => PrimitiveType::PrimitiveBigInt,
    Expression::TemplateLiteral(e) => {
      if e.expressions.is_empty() {
        PrimitiveType::PrimitiveString
      } else {
        PrimitiveType::PrimitiveUnknown
      }
    }
    Expression::UpdateExpression(e) => {
      match e.operator {
        UpdateOperator::Increment | UpdateOperator::Decrement => {
          PrimitiveType::PrimitiveMixed // Can be number or bigint
        }
      }
    }
    Expression::UnaryExpression(e) => match e.operator {
      UnaryOperator::Void => PrimitiveType::PrimitiveUndefined,
      UnaryOperator::Typeof => PrimitiveType::PrimitiveString,
      UnaryOperator::LogicalNot | UnaryOperator::Delete => PrimitiveType::PrimitiveBoolean,
      UnaryOperator::UnaryPlus => PrimitiveType::PrimitiveNumber, // Cannot be bigint because that throws an exception
      UnaryOperator::UnaryNegation | UnaryOperator::BitwiseNot => {
        let value = known_primitive_type(scope, &e.argument);
        if value == PrimitiveType::PrimitiveBigInt {
          return PrimitiveType::PrimitiveBigInt;
        }
        if value != PrimitiveType::PrimitiveUnknown && value != PrimitiveType::PrimitiveMixed {
          return PrimitiveType::PrimitiveNumber;
        }
        PrimitiveType::PrimitiveMixed // Can be number or bigint
      }
    },
    Expression::LogicalExpression(e) => match e.operator {
      LogicalOperator::Or | LogicalOperator::And => {
        merged_known_primitive_types(scope, &e.left, &e.right)
      }
      LogicalOperator::Coalesce => {
        let left = known_primitive_type(scope, &e.left);
        let right = known_primitive_type(scope, &e.right);
        if left == PrimitiveType::PrimitiveNull || left == PrimitiveType::PrimitiveUndefined {
          return right;
        }
        if left != PrimitiveType::PrimitiveUnknown {
          if left != PrimitiveType::PrimitiveMixed {
            return left; // Definitely not null or undefined
          }
          if right != PrimitiveType::PrimitiveUnknown {
            return PrimitiveType::PrimitiveMixed; // Definitely some kind of primitive
          }
        }
        PrimitiveType::PrimitiveUnknown
      }
    },
    Expression::BinaryExpression(e) => match e.operator {
      BinaryOperator::StrictEquality
      | BinaryOperator::StrictInequality
      | BinaryOperator::Equality
      | BinaryOperator::Inequality
      | BinaryOperator::LessThan
      | BinaryOperator::GreaterThan
      | BinaryOperator::LessThan
      | BinaryOperator::GreaterEqualThan
      | BinaryOperator::LessEqualThan
      | BinaryOperator::Instanceof
      | BinaryOperator::In => PrimitiveType::PrimitiveBoolean,
      BinaryOperator::Addition => {
        let left = known_primitive_type(scope, &e.left);
        let right = known_primitive_type(scope, &e.right);
        if left == PrimitiveType::PrimitiveString || right == PrimitiveType::PrimitiveString {
          PrimitiveType::PrimitiveString
        } else if left == PrimitiveType::PrimitiveBigInt && right == PrimitiveType::PrimitiveBigInt
        {
          PrimitiveType::PrimitiveBigInt
        } else if left != PrimitiveType::PrimitiveUnknown
          && left != PrimitiveType::PrimitiveMixed
          && left != PrimitiveType::PrimitiveBigInt
          && right != PrimitiveType::PrimitiveUnknown
          && right != PrimitiveType::PrimitiveMixed
          && right != PrimitiveType::PrimitiveBigInt
        {
          PrimitiveType::PrimitiveNumber
        } else {
          PrimitiveType::PrimitiveMixed // Can be number or bigint or string (or an exception)
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
      | BinaryOperator::BitwiseXOR => PrimitiveType::PrimitiveMixed,
    },

    Expression::AssignmentExpression(e) => match e.operator {
      oxc::syntax::operator::AssignmentOperator::Assign => known_primitive_type(scope, &e.right),
      oxc::syntax::operator::AssignmentOperator::Addition => {
        let right = known_primitive_type(scope, &e.right);
        if right == PrimitiveType::PrimitiveString {
          PrimitiveType::PrimitiveString
        } else {
          PrimitiveType::PrimitiveMixed // Can be number or bigint or string (or an exception)
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
      | oxc::syntax::operator::AssignmentOperator::Exponential => PrimitiveType::PrimitiveMixed,
    },
    _ => PrimitiveType::PrimitiveUnknown,
  }
}
