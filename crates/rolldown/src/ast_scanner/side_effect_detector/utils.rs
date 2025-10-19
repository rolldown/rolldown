use oxc::{
  ast::ast::{self, Expression, MemberExpression},
  semantic::ReferenceId,
  span::Atom,
  syntax::operator::{BinaryOperator, LogicalOperator, UnaryOperator, UpdateOperator},
};
use rolldown_common::AstScopes;
use rolldown_ecmascript_utils::ExpressionExt;

#[derive(PartialEq, Eq, Copy, Clone)]
pub enum PrimitiveType {
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

pub fn known_primitive_type(scope: &AstScopes, expr: &Expression) -> PrimitiveType {
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

pub fn can_change_strict_to_loose(scope: &AstScopes, a: &Expression, b: &Expression) -> bool {
  let x = known_primitive_type(scope, a);
  let y = known_primitive_type(scope, b);
  x == y && !matches!(x, PrimitiveType::Unknown | PrimitiveType::Mixed)
}

pub fn is_primitive_literal(scope: &AstScopes, expr: &Expression) -> bool {
  match expr {
    Expression::NullLiteral(_)
    | Expression::BooleanLiteral(_)
    | Expression::NumericLiteral(_)
    | Expression::StringLiteral(_)
    | Expression::BigIntLiteral(_) => true,
    // Include `+1` / `-1`.
    Expression::UnaryExpression(e) => match e.operator {
      UnaryOperator::Void => is_primitive_literal(scope, &e.argument),
      UnaryOperator::UnaryNegation | UnaryOperator::UnaryPlus => {
        matches!(e.argument, Expression::NumericLiteral(_))
      }
      _ => false,
    },
    Expression::Identifier(id)
      if id.name == "undefined" && scope.is_unresolved(id.reference_id()) =>
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
  let mut cur = match expr {
    MemberExpression::ComputedMemberExpression(computed_expr) => {
      let Expression::StringLiteral(ref str) = computed_expr.expression else {
        return None;
      };
      chain.push(str.value);
      &computed_expr.object
    }
    MemberExpression::StaticMemberExpression(static_expr) => {
      chain.push(static_expr.property.name);
      &static_expr.object
    }
    MemberExpression::PrivateFieldExpression(_) => return None,
  };

  // extract_rest_member_expr_chain
  loop {
    match cur {
      Expression::StaticMemberExpression(expr) => {
        cur = &expr.object;
        chain.push(expr.property.name);
      }
      Expression::ComputedMemberExpression(expr) => {
        let Expression::StringLiteral(ref str) = expr.expression else {
          break;
        };
        chain.push(str.value);
        cur = &expr.object;
      }
      Expression::Identifier(ident) => {
        chain.push(ident.name);
        let ref_id = ident.reference_id.get().expect("should have reference_id");
        chain.reverse();
        return Some((ref_id, chain));
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
) -> Option<bool> {
  let ident = value.as_identifier()?;
  let is_unresolved = scope.is_unresolved(ident.reference_id());
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
) -> bool {
  let Some(ident) = expr.callee.as_identifier() else {
    return false;
  };

  if scope.is_unresolved(ident.reference_id()) {
    match ident.name.as_str() {
      // TypedArray constructors - considered side-effect free with no args, null, or undefined
      "Int8Array" | "Uint8Array" | "Uint8ClampedArray" | "Int16Array" | "Uint16Array"
      | "Int32Array" | "Uint32Array" | "Float32Array" | "Float64Array" | "BigInt64Array"
      | "BigUint64Array" => match expr.arguments.len() {
        0 => return true,
        1 => {
          let arg = &expr.arguments[0];
          match arg {
            ast::Argument::NullLiteral(_) => return true,
            ast::Argument::Identifier(id)
              if id.name == "undefined" && scope.is_unresolved(id.reference_id()) =>
            {
              return true;
            }
            _ => {}
          }
        }
        _ => {}
      },
      "WeakSet" | "WeakMap" => match expr.arguments.len() {
        0 => return true,
        1 => {
          let arg = &expr.arguments[0];
          match arg {
            ast::Argument::NullLiteral(_) => return true,
            ast::Argument::Identifier(id)
              if id.name == "undefined" && scope.is_unresolved(id.reference_id()) =>
            {
              return true;
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
            arg.as_expression().map(|item| known_primitive_type(scope, item));
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
              if id.name == "undefined" && scope.is_unresolved(id.reference_id()) =>
            {
              return true;
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
              if id.name == "undefined" && scope.is_unresolved(id.reference_id()) =>
            {
              return true;
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
