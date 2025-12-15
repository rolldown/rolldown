use arcstr::ArcStr;
use oxc::{
  allocator::Allocator,
  ast::AstBuilder,
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
    let expr = json_value_to_expression(value, builder);
    let stmt = builder.statement_expression(SPAN, expr);

    builder.program(
      SPAN,
      SourceType::default().with_module(true),
      "",
      builder.vec(),
      None,
      builder.vec(),
      builder.vec1(stmt),
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
  builder: AstBuilder<'a>,
) -> oxc::ast::ast::Expression<'a> {
  match value {
    Value::Null => builder.expression_null_literal(SPAN),

    Value::Bool(b) => builder.expression_boolean_literal(SPAN, *b),

    Value::Number(n) => {
      // serde_json::Number can always be represented as f64 for JSON numbers.
      // Large integers may lose precision, matching JavaScript's JSON.parse behavior.
      let f = n.as_f64().expect("JSON numbers are always representable as f64");
      builder.expression_numeric_literal(SPAN, f, None, oxc::ast::ast::NumberBase::Decimal)
    }

    Value::String(s) => builder.expression_string_literal(SPAN, builder.atom(s), None),

    Value::Array(arr) => {
      let elements = builder.vec_from_iter(arr.iter().map(|item| {
        let expr = json_value_to_expression(item, builder);
        oxc::ast::ast::ArrayExpressionElement::from(expr)
      }));
      builder.expression_array(SPAN, elements)
    }

    Value::Object(obj) => {
      let properties = builder.vec_from_iter(obj.iter().map(|(key, val)| {
        let key_expr = builder.expression_string_literal(SPAN, builder.atom(key), None);
        let value_expr = json_value_to_expression(val, builder);

        builder.object_property_kind_object_property(
          SPAN,
          oxc::ast::ast::PropertyKind::Init,
          oxc::ast::ast::PropertyKey::from(key_expr),
          value_expr,
          false, // shorthand
          false, // computed
          false, // method
        )
      }));
      builder.expression_object(SPAN, properties)
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
