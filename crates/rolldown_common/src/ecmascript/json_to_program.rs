use arcstr::ArcStr;
use oxc::{
  allocator::Allocator,
  ast::{ast, builder::AstBuilder},
  ast_visit::{VisitJsMut, walk_js_mut},
  span::{SPAN, SourceType, Span},
};
use rolldown_ecmascript::{EcmaAst, EcmaCompiler};
use rustc_hash::FxHashMap;
use serde_json::Value;

/// Parses a JSON module into its expression statement.
pub fn parse_json_to_ecma_ast(source: &str) -> Result<EcmaAst, serde_json::Error> {
  match serde_json::from_str(source) {
    Ok(value) => Ok(json_value_to_ecma_ast(&value)),
    Err(error) => parse_json_with_surrogates(source, error),
  }
}

#[cold]
fn parse_json_with_surrogates(
  source: &str,
  error: serde_json::Error,
) -> Result<EcmaAst, serde_json::Error> {
  // Validate a copy with surrogate escapes replaced by same-length scalar escapes. This keeps
  // serde_json's grammar and parser limits without requiring its UTF-8 strings to hold surrogates.
  // The copy is never emitted: the JavaScript parser below reads the original UTF-16 strings.
  // See internal-docs/json-modules/implementation.md.
  let mut validation_source = source.as_bytes().to_vec();
  let mut index = 0;
  while index + 1 < validation_source.len() {
    if validation_source[index] != b'\\' {
      index += 1;
      continue;
    }
    if validation_source[index + 1] == b'u'
      && index + 5 < validation_source.len()
      && matches!(validation_source[index + 2], b'd' | b'D')
      && matches!(validation_source[index + 3], b'8'..=b'9' | b'a'..=b'f' | b'A'..=b'F')
      && validation_source[index + 4..index + 6].iter().all(u8::is_ascii_hexdigit)
    {
      validation_source[index + 2] = b'e';
    }
    // Consume the escaped character as well, so \\ud800 remains a literal backslash and text.
    index += 2;
  }
  if serde_json::from_slice::<Value>(&validation_source).is_err() {
    return Err(error);
  }
  let Ok(mut ast) = EcmaCompiler::parse_expr_as_program("", source, SourceType::default()) else {
    return Err(error);
  };
  ast.program.with_mut(|fields| {
    JsonNormalizer { allocator: fields.allocator }.visit_program(fields.program);
  });
  Ok(ast)
}

struct JsonNormalizer<'a> {
  allocator: &'a Allocator,
}

impl<'a> VisitJsMut<'a> for JsonNormalizer<'a> {
  fn visit_span(&mut self, span: &mut Span) {
    *span = SPAN;
  }

  fn visit_object_expression(&mut self, object: &mut ast::ObjectExpression<'a>) {
    walk_js_mut::walk_object_expression(self, object);
    let properties =
      std::mem::replace(&mut object.properties, oxc::allocator::Vec::new_in(&self.allocator));
    let mut indices = FxHashMap::default();
    for mut property in properties {
      let ast::ObjectPropertyKind::ObjectProperty(object_property) = &mut property else {
        unreachable!("JSON objects only contain properties");
      };
      let ast::PropertyKey::StringLiteral(key) = &object_property.key else {
        unreachable!("JSON property names are strings");
      };
      let name = key.value;
      // JSON.parse creates an own data property, including for escaped __proto__ spellings.
      object_property.computed = name == "__proto__";
      if let Some(&index) = indices.get(&name) {
        // Duplicate keys retain their first insertion position and their last value.
        object.properties[index] = property;
      } else {
        indices.insert(name, object.properties.len());
        object.properties.push(property);
      }
    }
  }
}

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
      [],
      None,
      [],
      [stmt],
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
            // JSON treats this as an own data property, not an object literal's prototype setter.
            key == "__proto__", // computed
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

  #[test]
  fn test_json_string_code_units() {
    for (json, expected) in [
      (r#""\ud800""#, vec![0xd800]),
      (r#""\uD8aF""#, vec![0xd8af]),
      (r#""\udfff""#, vec![0xdfff]),
      (r#""\uDfFf""#, vec![0xdfff]),
      (r#""\ud800\ud800\udc00""#, vec![0xd800, 0xd800, 0xdc00]),
      (r#""\ud83d\ude00""#, vec![0xd83d, 0xde00]),
      (r#""\\ud800""#, "\\ud800".encode_utf16().collect()),
    ] {
      let ast = parse_json_to_ecma_ast(json).unwrap();
      let oxc::ast::ast::Statement::ExpressionStatement(statement) = &ast.program().body[0] else {
        panic!("JSON must produce an expression statement");
      };
      let oxc::ast::ast::Expression::StringLiteral(string) = &statement.expression else {
        panic!("JSON string must produce a string literal");
      };
      assert_eq!(string.value.encode_utf16().collect::<Vec<_>>(), expected, "{json}");
    }
  }

  #[test]
  fn test_json_surrogate_keys_and_duplicate_properties() {
    let ast = parse_json_to_ecma_ast(
      r#"{"\ud800":1,"kept":true,"\uD800":2,"\u005f_proto__":0,"__proto__":{"text":"\udfff"}}"#,
    )
    .unwrap();
    let ast::Statement::ExpressionStatement(statement) = &ast.program().body[0] else {
      panic!("JSON must produce an expression statement");
    };
    let ast::Expression::ObjectExpression(object) = &statement.expression else {
      panic!("JSON object must produce an object expression");
    };
    let properties =
      object.properties.iter().map(|property| property.as_property().unwrap()).collect::<Vec<_>>();
    assert_eq!(properties.len(), 3);
    let ast::PropertyKey::StringLiteral(first_key) = &properties[0].key else {
      panic!("JSON property names must remain strings");
    };
    assert_eq!(first_key.value.encode_utf16().collect::<Vec<_>>(), [0xd800]);
    let ast::Expression::NumericLiteral(number) = &properties[0].value else {
      panic!("the last duplicate value must survive");
    };
    assert_eq!(number.value, 2.0);
    assert!(properties[1].key.is_specific_static_name("kept"));
    assert!(properties[2].key.is_specific_static_name("__proto__"));
    assert!(properties[2].computed);
  }

  #[test]
  fn test_json_overwritten_surrogate_values() {
    for json in [r#"{"value":"\ud800","value":1}"#, r#"{"value":{"nested":["\udfff"]},"value":1}"#]
    {
      let ast = parse_json_to_ecma_ast(json).unwrap();
      assert_eq!(Codegen::new().build(ast.program()).code, "({ \"value\": 1 });\n");
    }
  }

  #[test]
  fn test_json_surrogates_do_not_allow_javascript_syntax() {
    for json in [
      r#"{"text":"\ud800",}"#,
      r#"{"text":"\ud800","value":undefined}"#,
      r#"{"text":"\ud800","value":NaN}"#,
      r#"{"text":"\ud800","value":Infinity}"#,
      r#"{"text":"\ud800","value":+1}"#,
      r#"{"text":"\ud800","value":01}"#,
      r#"{"text":"\ud800","value":0x1}"#,
      r#"{"text":"\ud800","value":'single quotes'}"#,
      r#"{"text":"\ud800", unquoted:1}"#,
      r#"{"text":"\ud800", /* comment */ "value":1}"#,
      r#"{"text":"\ud800","value":()=>1}"#,
      r#"["\ud800",,1]"#,
      r#"("\ud800")"#,
      r#""\ud800"; null"#,
      r#""\ud800\q""#,
      r#""\ud800\uZZZZ""#,
      r#""\ud800\uD8GG""#,
      "\"\\ud800\n\"",
    ] {
      assert!(parse_json_to_ecma_ast(json).is_err(), "must reject {json:?}");
    }
  }

  #[test]
  fn test_json_other_parse_errors_are_preserved() {
    for value in ["0", r#""\ud800""#] {
      let json = format!("{}{value}{}", "[".repeat(150), "]".repeat(150));
      assert_eq!(
        parse_json_to_ecma_ast(&json).unwrap_err().to_string(),
        serde_json::from_str::<Value>(&json).unwrap_err().to_string()
      );
    }
  }

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
    	"pos": 1e400,
    	"neg": -1e400
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
