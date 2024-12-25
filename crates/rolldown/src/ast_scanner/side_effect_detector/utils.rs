use oxc::{
  ast::{
    ast::{self, Expression, MemberExpression},
    comments_range, Comment, CommentKind,
  },
  semantic::{ReferenceId, SymbolTable},
  span::{Atom, Span},
  syntax::operator::{BinaryOperator, LogicalOperator, UnaryOperator, UpdateOperator},
};
use rolldown_common::AstScopes;
use rolldown_ecmascript_utils::{ExpressionExt, SpanExt};

use super::SideEffectDetector;

impl SideEffectDetector<'_> {
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
    let comment = comments_range(self.comments, ..span.start).next_back()?;

    let comment_span = comment.content_span();

    let comment_text = comment_span.source_text(self.source);
    // If there are non-whitespace characters between the `comment` and the `span`,
    // we treat the `comment` not belongs to the `span`.
    let leading_comment_span = Span::new(comment_span.end, span.start);
    if !leading_comment_span.is_valid(self.source) {
      return None;
    }
    let range_text = leading_comment_span.source_text(self.source);
    let only_whitespace = match comment.kind {
      CommentKind::Line => range_text.trim().is_empty(),
      CommentKind::Block => {
        range_text
          .strip_prefix("*/") // for multi-line comment
          .is_some_and(|s| s.trim().is_empty())
      }
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
  symbol_table: &SymbolTable,
) -> PrimitiveType {
  let left_type = known_primitive_type(scope, left, symbol_table);
  if left_type == PrimitiveType::Unknown {
    return PrimitiveType::Unknown;
  }
  let right_type = known_primitive_type(scope, right, symbol_table);
  if right_type == PrimitiveType::Unknown {
    return PrimitiveType::Unknown;
  }
  if right_type == left_type {
    return right_type;
  }
  PrimitiveType::Mixed
}

#[allow(clippy::too_many_lines)]
pub(crate) fn known_primitive_type(
  scope: &AstScopes,
  expr: &Expression,
  symbol_table: &SymbolTable,
) -> PrimitiveType {
  match expr {
    Expression::NullLiteral(_) => PrimitiveType::Null,
    Expression::Identifier(id)
      if id.name == "undefined"
        && scope.is_unresolved(id.reference_id.get().unwrap(), symbol_table) =>
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
        let value = known_primitive_type(scope, &e.argument, symbol_table);
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
        merged_known_primitive_types(scope, &e.left, &e.right, symbol_table)
      }
      LogicalOperator::Coalesce => {
        let left = known_primitive_type(scope, &e.left, symbol_table);
        let right = known_primitive_type(scope, &e.right, symbol_table);
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
        let left = known_primitive_type(scope, &e.left, symbol_table);
        let right = known_primitive_type(scope, &e.right, symbol_table);
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
      oxc::syntax::operator::AssignmentOperator::Assign => {
        known_primitive_type(scope, &e.right, symbol_table)
      }
      oxc::syntax::operator::AssignmentOperator::Addition => {
        let right = known_primitive_type(scope, &e.right, symbol_table);
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

pub fn can_change_strict_to_loose(
  scope: &AstScopes,
  a: &Expression,
  b: &Expression,
  symbol_table: &SymbolTable,
) -> bool {
  let x = known_primitive_type(scope, a, symbol_table);
  let y = known_primitive_type(scope, b, symbol_table);
  x == y && !matches!(x, PrimitiveType::Unknown | PrimitiveType::Mixed)
}

pub fn is_primitive_literal(
  scope: &AstScopes,
  expr: &Expression,
  symbol_table: &SymbolTable,
) -> bool {
  match expr {
    Expression::NullLiteral(_)
    | Expression::BooleanLiteral(_)
    | Expression::NumericLiteral(_)
    | Expression::StringLiteral(_)
    | Expression::BigIntLiteral(_) => true,
    // Include `+1` / `-1`.
    Expression::UnaryExpression(e)
      if matches!(e.operator, |UnaryOperator::UnaryNegation| UnaryOperator::UnaryPlus)
        && matches!(e.argument, Expression::NumericLiteral(_)) =>
    {
      true
    }
    Expression::Identifier(id)
      if id.name == "undefined"
        && scope.is_unresolved(id.reference_id.get().unwrap(), symbol_table) =>
    {
      true
    }
    _ => false,
  }
}

pub fn extract_member_expr_chain<'a>(
  expr: &'a MemberExpression,
  max_len: usize,
) -> Option<(ReferenceId, Vec<Atom<'a>>)> {
  if max_len == 0 {
    return None;
  }
  let mut chain = vec![];
  match expr {
    MemberExpression::ComputedMemberExpression(computed_expr) => {
      let Expression::StringLiteral(ref str) = computed_expr.expression else {
        return None;
      };
      chain.push(str.value.clone());
      let mut cur = &computed_expr.object;
      extract_rest_member_expr_chain(&mut cur, &mut chain, max_len).map(|ref_id| (ref_id, chain))
    }
    MemberExpression::StaticMemberExpression(static_expr) => {
      let mut cur = &static_expr.object;
      chain.push(static_expr.property.name.clone());
      extract_rest_member_expr_chain(&mut cur, &mut chain, max_len).map(|ref_id| (ref_id, chain))
    }
    MemberExpression::PrivateFieldExpression(_) => None,
  }
}

fn extract_rest_member_expr_chain<'a>(
  cur: &mut &'a Expression,
  chain: &mut Vec<Atom<'a>>,
  max_len: usize,
) -> Option<ReferenceId> {
  loop {
    match &cur {
      Expression::StaticMemberExpression(expr) => {
        *cur = &expr.object;
        chain.push(expr.property.name.clone());
      }
      Expression::ComputedMemberExpression(expr) => {
        let Expression::StringLiteral(ref str) = expr.expression else {
          break;
        };
        chain.push(str.value.clone());
        *cur = &expr.object;
      }
      Expression::Identifier(ident) => {
        chain.push(ident.name.clone());
        let ref_id = ident.reference_id.get().expect("should have reference_id");
        chain.reverse();
        return Some(ref_id);
      }
      _ => break,
    }
    // If chain exceeds the max length, that means we are not interest in this member expression.
    // return `None`
    if chain.len() >= max_len {
      return None;
    }
  }
  None
}

/// https://github.com/evanw/esbuild/blob/d34e79e2a998c21bb71d57b92b0017ca11756912/internal/js_ast/js_ast_helpers.go#L2594-L2639
pub fn is_side_effect_free_unbound_identifier_ref(
  scope: &AstScopes,
  value: &Expression,
  guard_condition: &Expression,
  mut is_yes_branch: bool,
  symbol_table: &SymbolTable,
) -> Option<bool> {
  let ident = value.as_identifier()?;
  let is_unresolved = scope.is_unresolved(ident.reference_id(), symbol_table);
  if !is_unresolved {
    return Some(false);
  }
  let bin_expr = guard_condition.as_binary_expression()?;
  match bin_expr.operator {
    BinaryOperator::StrictEquality
    | BinaryOperator::StrictInequality
    | BinaryOperator::Equality
    | BinaryOperator::Inequality => {
      let (mut ty_of, mut string) = (&bin_expr.left, &bin_expr.right);
      if matches!(ty_of, Expression::StringLiteral(_)) {
        std::mem::swap(&mut string, &mut ty_of);
      }
      let unary = ty_of.as_unary_expression()?;
      if !(unary.operator == UnaryOperator::Typeof
        && matches!(unary.argument, Expression::Identifier(_)))
      {
        return Some(false);
      }
      let string = string.as_string_literal()?;

      if (string.value.eq("undefined") == is_yes_branch)
        == matches!(
          bin_expr.operator,
          BinaryOperator::Inequality | BinaryOperator::StrictInequality
        )
      {
        let type_of_value = unary.argument.as_identifier()?;
        if type_of_value.name == ident.name {
          return Some(true);
        }
      }
    }
    BinaryOperator::LessThan
    | BinaryOperator::LessEqualThan
    | BinaryOperator::GreaterThan
    | BinaryOperator::GreaterEqualThan => {
      let (mut ty_of, mut string) = (&bin_expr.left, &bin_expr.right);
      if matches!(ty_of, Expression::StringLiteral(_)) {
        std::mem::swap(&mut string, &mut ty_of);
        is_yes_branch = !is_yes_branch;
      }

      let unary = ty_of.as_unary_expression()?;
      if !(unary.operator == UnaryOperator::Typeof
        && matches!(unary.argument, Expression::Identifier(_)))
      {
        return Some(false);
      }

      let string = string.as_string_literal()?;

      if string.value == "u"
        && is_yes_branch
          == matches!(bin_expr.operator, BinaryOperator::LessThan | BinaryOperator::LessEqualThan)
      {
        let type_of_value = unary.argument.as_identifier()?;
        if type_of_value.name == ident.name {
          return Some(true);
        }
      }
    }
    _ => {}
  }
  Some(false)
}

/// https://github.com/evanw/esbuild/blob/d34e79e2a998c21bb71d57b92b0017ca11756912/internal/js_parser/js_parser.go#L16119-L16237
pub fn maybe_side_effect_free_global_constructor(
  scope: &AstScopes,
  expr: &ast::NewExpression<'_>,
  symbol_table: &oxc::semantic::SymbolTable,
) -> bool {
  let Some(ident) = expr.callee.as_identifier() else {
    return false;
  };

  if scope.is_unresolved(ident.reference_id(), symbol_table) {
    match ident.name.as_str() {
      "WeakSet" | "WeakMap" => match expr.arguments.len() {
        0 => return true,
        1 => {
          let arg = &expr.arguments[0];
          match arg {
            ast::Argument::NullLiteral(_) => return true,
            ast::Argument::Identifier(id)
              if id.name == "undefined" && scope.is_unresolved(id.reference_id(), symbol_table) =>
            {
              return true
            }
            ast::Argument::ArrayExpression(arr) if arr.elements.is_empty() => return true,
            _ => {}
          }
        }
        _ => {}
      },
      "Date" => match expr.arguments.len() {
        0 => return true,
        1 => {
          let arg = &expr.arguments[0];
          let known_primitive_type =
            arg.as_expression().map(|item| known_primitive_type(scope, item, symbol_table));
          if let Some(primitive_ty) = known_primitive_type {
            if matches!(
              primitive_ty,
              PrimitiveType::Number
                | PrimitiveType::String
                | PrimitiveType::Null
                | PrimitiveType::Undefined
                | PrimitiveType::Boolean
            ) {
              return true;
            }
          }
        }
        _ => {}
      },
      "Set" => match expr.arguments.len() {
        0 => return true,
        1 => {
          let arg = &expr.arguments[0];
          match arg {
            ast::Argument::NullLiteral(_) | ast::Argument::ArrayExpression(_) => return true,
            ast::Argument::Identifier(id)
              if id.name == "undefined" && scope.is_unresolved(id.reference_id(), symbol_table) =>
            {
              return true
            }
            _ => {}
          }
        }
        _ => {}
      },
      "Map" => match expr.arguments.len() {
        0 => return true,
        1 => {
          let arg = &expr.arguments[0];
          match arg {
            ast::Argument::NullLiteral(_) => return true,
            ast::Argument::Identifier(id)
              if id.name == "undefined" && scope.is_unresolved(id.reference_id(), symbol_table) =>
            {
              return true
            }
            ast::Argument::ArrayExpression(arr) => {
              let all_entries_are_arrays = arr.elements.iter().all(|item| {
                item
                  .as_expression()
                  .is_some_and(|expr| matches!(expr, ast::Expression::ArrayExpression(_)))
              });
              if all_entries_are_arrays {
                return true;
              }
            }
            _ => {}
          }
        }
        _ => {}
      },
      _ => {}
    }
  }
  false
}
