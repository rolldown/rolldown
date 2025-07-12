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
}

impl ConstExportMeta {
  pub fn new(value: ConstantValue, commonjs_export: bool) -> Self {
    Self { value, commonjs_export }
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

impl ConstantValue {
  pub fn to_expression<'ast>(&self, ast: oxc::ast::AstBuilder<'ast>) -> Expression<'ast> {
    match self {
      ConstantValue::Number(n) => {
        ast.expression_numeric_literal(SPAN, *n, None, oxc::ast::ast::NumberBase::Decimal)
      }
      ConstantValue::BigInt(b) => ast.expression_big_int_literal(
        SPAN,
        ast.atom(&b.to_string()),
        None,
        oxc::ast::ast::BigintBase::Decimal,
      ),
      ConstantValue::String(s) => ast.expression_string_literal(SPAN, ast.atom(s), None),
      ConstantValue::Boolean(b) => ast.expression_boolean_literal(SPAN, *b),
      ConstantValue::Undefined => ast.void_0(SPAN),
      ConstantValue::Null => ast.expression_null_literal(SPAN),
    }
  }
}
