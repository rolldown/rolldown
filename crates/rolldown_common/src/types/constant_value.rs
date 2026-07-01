use std::borrow::Cow;

use num_bigint::BigInt;
use oxc::{ast::ast::Expression, span::SPAN};
use oxc_ecmascript::constant_evaluation;

#[derive(Debug, PartialEq, Clone)]
pub enum ConstantValue {
  Number(f64),
  BigInt(BigInt),
  // TODO: Maybe need to store as a `Cow<'s>` string, but that
  // would populate the lifetime everywhere.
  String(String),
  Boolean(bool),
  Undefined,
  Null,
}

#[derive(Debug, Clone)]
pub struct ConstExportMeta {
  pub value: ConstantValue,
  /// For now we only support esm and commonjs format, so `bool` is enough.
  pub commonjs_export: bool,
  /// If `true`, it's safe to inline this constant value regardless of **inlineConst mode**
  pub safe_to_inline: bool,
}

impl ConstExportMeta {
  pub fn new(value: ConstantValue, commonjs_export: bool) -> Self {
    let safe_to_inline = match &value {
      ConstantValue::Number(n) => n.fract() == 0.0 && *n >= -99.0 && *n <= 999.0,
      ConstantValue::BigInt(_) => false,
      ConstantValue::String(s) => s.len() <= 3,
      ConstantValue::Boolean(_) | ConstantValue::Undefined | ConstantValue::Null => true,
    };
    Self { value, commonjs_export, safe_to_inline }
  }
}

impl From<constant_evaluation::ConstantValue<'_>> for ConstantValue {
  fn from(value: constant_evaluation::ConstantValue<'_>) -> Self {
    match value {
      constant_evaluation::ConstantValue::Number(n) => ConstantValue::Number(n),
      constant_evaluation::ConstantValue::BigInt(b) => ConstantValue::BigInt(b),
      constant_evaluation::ConstantValue::String(s) => ConstantValue::String(s.to_string()),
      constant_evaluation::ConstantValue::Boolean(b) => ConstantValue::Boolean(b),
      constant_evaluation::ConstantValue::Undefined => ConstantValue::Undefined,
      constant_evaluation::ConstantValue::Null => ConstantValue::Null,
    }
  }
}

impl From<&ConstantValue> for constant_evaluation::ConstantValue<'_> {
  fn from(value: &ConstantValue) -> Self {
    match value {
      ConstantValue::Number(n) => constant_evaluation::ConstantValue::Number(*n),
      ConstantValue::BigInt(b) => constant_evaluation::ConstantValue::BigInt(b.clone()),
      ConstantValue::String(s) => constant_evaluation::ConstantValue::String(Cow::Owned(s.clone())),
      ConstantValue::Boolean(b) => constant_evaluation::ConstantValue::Boolean(*b),
      ConstantValue::Undefined => constant_evaluation::ConstantValue::Undefined,
      ConstantValue::Null => constant_evaluation::ConstantValue::Null,
    }
  }
}

impl ConstantValue {
  pub fn to_expression<'ast>(&self, ast: oxc::ast::AstBuilder<'ast>) -> Expression<'ast> {
    match self {
      ConstantValue::Number(n) => {
        Expression::new_numeric_literal(SPAN, *n, None, oxc::ast::ast::NumberBase::Decimal, &ast)
      }
      ConstantValue::BigInt(b) => Expression::new_big_int_literal(
        SPAN,
        oxc::ast::ast::Str::from_str_in(&b.to_string(), &ast),
        None,
        oxc::ast::ast::BigintBase::Decimal,
        &ast,
      ),
      ConstantValue::String(s) => {
        Expression::new_string_literal(SPAN, oxc::ast::ast::Str::from_str_in(s, &ast), None, &ast)
      }
      ConstantValue::Boolean(b) => Expression::new_boolean_literal(SPAN, *b, &ast),
      ConstantValue::Undefined => Expression::new_void_0(SPAN, &ast),
      ConstantValue::Null => Expression::new_null_literal(SPAN, &ast),
    }
  }
}
