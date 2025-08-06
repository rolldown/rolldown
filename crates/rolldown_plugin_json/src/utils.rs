use std::fmt::Write as _;

use rolldown_utils::concat_string;
use serde_json::Value;

// Use `10kB` as a threshold for 'auto'
// https://v8.dev/blog/cost-of-javascript-2019#json
pub const THRESHOLD_SIZE: usize = 10 * 1000;

/// /\.json(?:$|\?)(?!commonjs-(?:proxy|external))/
pub fn is_json_ext(ext: &str) -> bool {
  if ext.ends_with(".json") {
    return true;
  }
  let Some(i) = memchr::memmem::rfind(ext.as_bytes(), b".json?") else {
    return false;
  };
  let postfix = &ext[i + 6..];
  postfix != "commonjs-proxy" && postfix != "commonjs-external"
}

/// SPECIAL_QUERY_RE = /[?&](?:worker|sharedworker|raw|url)\b/
pub fn is_special_query(ext: &str) -> bool {
  for i in memchr::memrchr2_iter(b'?', b'&', ext.as_bytes()) {
    let Some(after) = ext.get(i + 1..) else {
      continue;
    };

    let boundary = if after.starts_with("raw") || after.starts_with("url") {
      3usize
    } else if after.starts_with("worker") {
      6usize
    } else if after.starts_with("sharedworker") {
      12usize
    } else {
      continue;
    };

    // Test if match `\b`
    match after.get(boundary..=boundary).and_then(|c| c.bytes().next()) {
      Some(ch) if !ch.is_ascii_alphanumeric() && ch != b'_' => {
        return true;
      }
      None => return true,
      _ => {}
    }
  }
  false
}

#[inline]
pub fn strip_bom(code: &str) -> &str {
  code.strip_prefix("\u{FEFF}").unwrap_or(code)
}

#[inline]
fn serialize_value(value: &Value) -> Result<String, serde_json::Error> {
  let value_as_string = serde_json::to_string(value)?;
  if value_as_string.len() > THRESHOLD_SIZE && value.is_object() {
    let value = serde_json::to_string(&value_as_string)?;
    Ok(concat_string!("/*#__PURE__*/ JSON.parse(", value, ")"))
  } else {
    Ok(value_as_string)
  }
}

pub fn json_to_esm(data: &Value, named_exports: bool) -> String {
  if !named_exports || !data.is_object() {
    return concat_string!("export default ", data.to_string(), ";\n");
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
      writeln!(default_object_code, "  {key},").unwrap();
    } else {
      let key = serde_json::to_string(key).unwrap();
      writeln!(default_object_code, "  {key}: {value},").unwrap();
    }
  }

  // Remove the trailing ",\n"
  default_object_code.truncate(default_object_code.len() - 2);

  concat_string!(named_export_code, "export default {\n", default_object_code, "\n};")
}

#[cfg(test)]
mod test {
  use crate::utils::{is_json_ext, is_special_query, json_to_esm};

  #[test]
  fn json_ext() {
    assert!(is_json_ext("test.json"));
    assert!(is_json_ext("test.json?test=test&b=100"));
    assert!(is_json_ext("test.json?commonjs-prox"));
    assert!(is_json_ext("test.json?commonjs-externa"));

    assert!(!is_json_ext("test.json?commonjs-proxy"));
    assert!(!is_json_ext("test.json?commonjs-external"));
  }

  #[test]
  fn special_query() {
    assert!(is_special_query("test?workers&worker"));
    assert!(is_special_query("test?url&sharedworker"));
    assert!(is_special_query("test?url&raw"));

    assert!(!is_special_query("test?&woer"));
    assert!(!is_special_query("test?&sharedworker1"));
  }

  #[test]
  fn to_esm_named_exports_object() {
    let data = serde_json::json!({"name": "name"});
    assert_eq!(
      "export const name = \"name\";\nexport default {\n  name\n};",
      json_to_esm(&data, true)
    );
  }

  #[test]
  fn to_esm_named_exports_literal() {
    let data = serde_json::json!(1);
    assert_eq!("export default 1;\n", json_to_esm(&data, true));
  }

  #[test]
  fn to_esm_named_exports_forbidden_ident() {
    let data = serde_json::json!({"true": true, "\\\"\n": 1234});
    assert_eq!(
      "export default {\n  \"true\": true,\n  \"\\\\\\\"\\n\": 1234\n};",
      json_to_esm(&data, true)
    );
  }

  #[test]
  fn to_esm_named_exports_multiple_fields() {
    let data = serde_json::json!({"foo": "foo", "bar": "bar"});
    assert_eq!(
      "export const foo = \"foo\";\nexport const bar = \"bar\";\nexport default {\n  foo,\n  bar\n};",
      json_to_esm(&data, true)
    );
  }
}
