use std::fmt::Write as _;

use json_escape_simd::escape;
use memchr::memmem;
use rolldown_utils::concat_string;
use serde_json::Value;

use super::constants::THRESHOLD_SIZE;

#[inline]
fn serialize_value(value: &Value) -> Result<String, serde_json::Error> {
  let value_as_string = serde_json::to_string(value)?;
  if value_as_string.len() > THRESHOLD_SIZE && value.is_object() {
    let value = escape(&value_as_string);
    Ok(concat_string!("/*#__PURE__*/ JSON.parse(", value, ")"))
  } else {
    Ok(json_to_js_expression(value_as_string))
  }
}

fn json_to_js_expression(json: String) -> String {
  const PROTO_KEY: &str = "\"__proto__\":";
  // The input is compact JSON from serde_json. An object delimiter before the key
  // distinguishes it from an escaped quote inside another key or string value.
  let mut keys = memmem::find_iter(json.as_bytes(), PROTO_KEY.as_bytes())
    .filter(|&start| start > 0 && matches!(json.as_bytes()[start - 1], b'{' | b','))
    .peekable();
  if keys.peek().is_none() {
    return json;
  }

  let mut code = String::with_capacity(json.len() + 2);
  let mut end = 0;
  for start in keys {
    code.push_str(&json[end..start]);
    // A computed key preserves the own data property at every nesting level.
    code.push_str("[\"__proto__\"]:");
    end = start + PROTO_KEY.len();
  }
  code.push_str(&json[end..]);
  code
}

pub fn data_to_esm(data: &Value, named_exports: bool) -> String {
  if !named_exports || !data.is_object() {
    return concat_string!("export default ", json_to_js_expression(data.to_string()), ";\n");
  }

  let data = data.as_object().unwrap();
  if data.is_empty() {
    return "export default {};\n".to_string();
  }

  let mut named_export_code = String::new();
  let mut default_object_code = String::new();
  for (key, value) in data {
    let value = serialize_value(value).expect("Invalid JSON value");
    if rolldown_utils::ecmascript::is_validate_assignee_identifier_name(key) {
      writeln!(named_export_code, "export const {key} = {value};").unwrap();
      if key == "__proto__" {
        writeln!(default_object_code, "  [\"__proto__\"]: __proto__,").unwrap();
      } else {
        writeln!(default_object_code, "  {key},").unwrap();
      }
    } else {
      let key = escape(key);
      writeln!(default_object_code, "  {key}: {value},").unwrap();
    }
  }

  // Remove the trailing ",\n"
  default_object_code.truncate(default_object_code.len() - 2);

  concat_string!(named_export_code, "export default {\n", default_object_code, "\n};")
}

#[cfg(test)]
mod test {
  use super::data_to_esm;

  #[test]
  fn to_esm_named_exports_object() {
    let data = serde_json::json!({"name": "name"});
    assert_eq!(
      "export const name = \"name\";\nexport default {\n  name\n};",
      data_to_esm(&data, true)
    );
  }

  #[test]
  fn to_esm_named_exports_literal() {
    let data = serde_json::json!(1);
    assert_eq!("export default 1;\n", data_to_esm(&data, true));
  }

  #[test]
  fn to_esm_named_exports_forbidden_ident() {
    let data = serde_json::json!({"true": true, "\\\"\n": 1234, "eval": 0, "arguments": 0});
    assert_eq!(
      "export default {\n  \"true\": true,\n  \"\\\\\\\"\\n\": 1234,\n  \"eval\": 0,\n  \"arguments\": 0\n};",
      data_to_esm(&data, true)
    );
  }

  #[test]
  fn to_esm_named_exports_multiple_fields() {
    let data = serde_json::json!({"foo": "foo", "bar": "bar"});
    assert_eq!(
      "export const foo = \"foo\";\nexport const bar = \"bar\";\nexport default {\n  foo,\n  bar\n};",
      data_to_esm(&data, true)
    );
  }

  #[test]
  fn to_esm_named_exports_large_object() {
    let data = serde_json::json!({"foo": {"bar": "\"\\\n\t\u{1}é".repeat(2000)}});
    let foo = serde_json::to_string(&data["foo"]).unwrap();
    let foo = serde_json::to_string(&foo).unwrap();
    assert_eq!(
      format!("export const foo = /*#__PURE__*/ JSON.parse({foo});\nexport default {{\n  foo\n}};"),
      data_to_esm(&data, true)
    );
  }

  #[test]
  fn to_esm_proto_data_properties() {
    for json in [
      r#"{"__proto__":null}"#,
      r#"{"__proto__":42}"#,
      r#"{"__proto__":{"nested":true}}"#,
      r#"{"nested":{"__proto__":false}}"#,
      r#"[{"__proto__":"value"}]"#,
    ] {
      let data: serde_json::Value = serde_json::from_str(json).unwrap();
      let code = data_to_esm(&data, false);
      assert!(code.contains(r#"["__proto__"]:"#), "{code}");
    }
  }

  #[test]
  fn to_esm_proto_with_escaped_keys_and_strings() {
    let data = serde_json::json!({"prefix\"__proto__": {"__proto__": 42}});
    assert_eq!(
      "export default {\"prefix\\\"__proto__\":{[\"__proto__\"]:42}};\n",
      data_to_esm(&data, false)
    );

    for json in [r#"{"__proto__":"literal"}"#, r#"x,{"__proto__":null}"#, r#"\"__proto__":"#] {
      let data = serde_json::Value::String(json.to_string());
      assert_eq!(format!("export default {data};\n"), data_to_esm(&data, false));
    }
  }

  #[test]
  fn to_esm_proto_does_not_reference_shadowed_json() {
    let data = serde_json::json!({"JSON": null, "nested": {"__proto__": 42}});
    let code = data_to_esm(&data, true);
    assert!(code.contains("export const JSON = null;"));
    assert!(code.contains("export const nested = {[\"__proto__\"]:42};"));
    assert!(!code.contains("JSON.parse"));
  }

  #[test]
  fn to_esm_named_proto_property() {
    let data = serde_json::json!({"__proto__": 42});
    assert_eq!(
      "export const __proto__ = 42;\nexport default {\n  [\"__proto__\"]: __proto__\n};",
      data_to_esm(&data, true)
    );
  }
}
