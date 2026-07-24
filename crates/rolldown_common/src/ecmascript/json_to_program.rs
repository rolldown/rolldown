use arcstr::ArcStr;
use oxc::{
  allocator::Allocator,
  ast::builder::AstBuilder,
  span::{SPAN, SourceType},
};
use rolldown_ecmascript::EcmaAst;
use serde_json::Value;

/// Converts a `serde_json::Value` to an `EcmaAst`.
///
/// The resulting AST contains a single expression statement with the JSON value
/// converted to its JavaScript AST equivalent.
///
/// # Arguments
/// * `value` - The JSON value to convert
///
/// # Returns
/// An `EcmaAst` containing the converted program
///
/// # Example
/// ```ignore
/// use serde_json::json;
/// use rolldown_common::json_value_to_ecma_ast;
///
/// let value = json!({"name": "test", "count": 42});
/// let ast = json_value_to_ecma_ast(&value);
/// ```
pub fn json_value_to_ecma_ast(value: &Value) -> EcmaAst {
  let source = ArcStr::from("");
  let allocator = Allocator::default();

  EcmaAst::from_allocator_and_source(source, allocator, |allocator| {
    let builder = AstBuilder::new(allocator);
    let expr = json_value_to_expression(value, &builder);
    let stmt = oxc::ast::ast::Statement::new_expression_statement(SPAN, expr, &builder);

    oxc::ast::ast::Program::new(
      SPAN,
      SourceType::default().with_module(true),
      "",
      oxc::allocator::Vec::new_in(&builder),
      None,
      oxc::allocator::Vec::new_in(&builder),
      oxc::allocator::Vec::from_value_in(stmt, &builder),
      &builder,
    )
  })
}

/// Converts a `serde_json::Value` to an oxc `Expression`.
///
/// This is useful when you need just the expression without wrapping it in a program.
///
/// # Arguments
/// * `value` - The JSON value to convert
/// * `builder` - The AST builder to use for node creation
///
/// # Returns
/// An `Expression` representing the JSON value
pub fn json_value_to_expression<'a>(
  value: &Value,
  builder: &AstBuilder<'a>,
) -> oxc::ast::ast::Expression<'a> {
  match value {
    Value::Null => oxc::ast::ast::Expression::new_null_literal(SPAN, builder),

    Value::Bool(b) => oxc::ast::ast::Expression::new_boolean_literal(SPAN, *b, builder),

    Value::Number(n) => {
      // A JSON number string is always a valid f64 literal, so parsing it never fails:
      // in-range values parse exactly (large integers may lose precision, like JS
      // `JSON.parse`), and out-of-range values like `1e400` saturate to a correctly
      // signed `±Infinity`. `as_f64` can't be used here because it filters out those
      // non-finite results and returns `None`, which is what used to panic.
      let f = n.as_str().parse::<f64>().expect("a JSON number is always a valid f64 literal");
      oxc::ast::ast::Expression::new_numeric_literal(
        SPAN,
        f,
        None,
        oxc::ast::ast::NumberBase::Decimal,
        builder,
      )
    }

    Value::String(s) => oxc::ast::ast::Expression::new_string_literal(
      SPAN,
      oxc::ast::ast::Str::from_str_in(s, builder),
      None,
      builder,
    ),

    Value::Array(arr) => {
      let elements = oxc::allocator::Vec::from_iter_in(
        arr.iter().map(|item| {
          let expr = json_value_to_expression(item, builder);
          oxc::ast::ast::ArrayExpressionElement::from(expr)
        }),
        builder,
      );
      oxc::ast::ast::Expression::new_array_expression(SPAN, elements, builder)
    }

    Value::Object(obj) => {
      let properties = oxc::allocator::Vec::from_iter_in(
        obj.iter().map(|(key, val)| {
          let key_expr = oxc::ast::ast::PropertyKey::new_string_literal(
            SPAN,
            oxc::ast::ast::Str::from_str_in(key, builder),
            None,
            builder,
          );
          let value_expr = json_value_to_expression(val, builder);

          oxc::ast::ast::ObjectPropertyKind::new_object_property(
            SPAN,
            oxc::ast::ast::PropertyKind::Init,
            key_expr,
            value_expr,
            false, // method
            false, // shorthand
            false, // computed
            builder,
          )
        }),
        builder,
      );
      oxc::ast::ast::Expression::new_object_expression(SPAN, properties, builder)
    }
  }
}

#[cfg(test)]
mod tests {
  use insta::assert_snapshot;
  use oxc::codegen::Codegen;

  use super::*;

  fn to_code(value: &Value) -> String {
    let ast = json_value_to_ecma_ast(value);
    Codegen::new().build(ast.program()).code
  }

  #[test]
  fn test_primitives() {
    assert_snapshot!(to_code(&serde_json::json!(null)), @"null;");
    assert_snapshot!(to_code(&serde_json::json!(true)), @"true;");
    assert_snapshot!(to_code(&serde_json::json!(false)), @"false;");
    assert_snapshot!(to_code(&serde_json::json!(42)), @"42;");
    assert_snapshot!(to_code(&serde_json::json!(3.5)), @"3.5;");
    assert_snapshot!(to_code(&serde_json::json!(-17)), @"-17;");
    assert_snapshot!(to_code(&serde_json::json!(0)), @"0;");
    assert_snapshot!(to_code(&serde_json::json!("hello")), @r#"("hello");"#);
    assert_snapshot!(to_code(&serde_json::json!("")), @r#"("");"#);
    assert_snapshot!(to_code(&serde_json::json!("with \"quotes\"")), @r#"("with \"quotes\"");"#);
  }

  /// Large integers beyond MAX_SAFE_INTEGER lose precision when parsed as f64.
  /// This matches JavaScript's `JSON.parse` behavior:
  /// `JSON.parse('{ "v": 9007199254740995 }').v` returns `9007199254740996`
  #[test]
  fn test_large_integer_precision_loss() {
    // 9007199254740995 is beyond Number.MAX_SAFE_INTEGER (2^53 - 1 = 9007199254740991)
    // When parsed as f64 and back, it becomes 9007199254740996
    let json: Value = serde_json::from_str(r#"{ "v": 9007199254740995 }"#).unwrap();
    assert_snapshot!(to_code(&json), @r#"({ "v": 9007199254740996 });"#);
  }

  /// Regression test: numbers outside f64's finite range must not panic. With serde_json's
  /// `arbitrary_precision` feature they parse successfully but `as_f64` returns `None`; we
  /// saturate to a correctly-signed `±Infinity`, matching JS
  /// `JSON.parse('{ "v": 1e400 }').v === Infinity`.
  #[test]
  fn test_out_of_range_number_saturates_to_infinity() {
    let json: Value = serde_json::from_str(r#"{ "pos": 1e400, "neg": -1e400 }"#).unwrap();
    assert_snapshot!(to_code(&json), @r#"
    ({
    	"pos": Infinity,
    	"neg": -Infinity
    });
    "#);
  }

  #[test]
  fn test_array() {
    assert_snapshot!(to_code(&serde_json::json!([])), @"[];");
    assert_snapshot!(to_code(&serde_json::json!([1, 2, 3])), @r"
    [
    	1,
    	2,
    	3
    ];
    ");
    assert_snapshot!(to_code(&serde_json::json!(["a", "b"])), @r#"["a", "b"];"#);
    assert_snapshot!(to_code(&serde_json::json!([1, "mixed", true, null])), @r#"
    [
    	1,
    	"mixed",
    	true,
    	null
    ];
    "#);
  }

  #[test]
  fn test_object() {
    assert_snapshot!(to_code(&serde_json::json!({})), @"({});");
    assert_snapshot!(to_code(&serde_json::json!({"a": 1})), @r#"({ "a": 1 });"#);
    assert_snapshot!(to_code(&serde_json::json!({"key with spaces": 1})), @r#"({ "key with spaces": 1 });"#);
    assert_snapshot!(to_code(&serde_json::json!({"true": 1})), @r#"({ "true": 1 });"#);
    // Note: serde_json deduplicates keys, keeping the last value
    assert_snapshot!(to_code(&serde_json::from_str::<Value>(r#"{"a": 1, "a": 2}"#).unwrap()), @r#"({ "a": 2 });"#);
  }

  /// Regression test for https://github.com/vitejs/vite/issues/21982
  #[test]
  fn test_float_17_significant_digits() {
    let inputs = [
      114.351_437_992_579_97_f64,
      406.314_867_132_489_95_f64,
      163.414_980_184_984_98_f64,
      364.094_987_249_009_9_f64,
    ];
    let json: Value = serde_json::from_str(
      r"[114.35143799257997, 406.31486713248995, 163.41498018498498, 364.09498724900986]",
    )
    .unwrap();
    let code: String = to_code(&json).chars().filter(|c| !c.is_whitespace()).collect();
    let expected = format!("[{}];", inputs.map(|v| v.to_string()).join(","));
    assert_eq!(code, expected);
  }

  #[test]
  fn test_nested() {
    assert_snapshot!(to_code(&serde_json::json!({
      "name": "test",
      "values": [1, 2, 3],
      "nested": {
        "deep": true
      }
    })), @r#"
    ({
    	"name": "test",
    	"values": [
    		1,
    		2,
    		3
    	],
    	"nested": { "deep": true }
    });
    "#);
  }
}
