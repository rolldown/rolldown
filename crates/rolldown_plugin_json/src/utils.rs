// cSpell:disable
use std::borrow::Cow;
use std::fmt::Write as _;

use rolldown_utils::concat_string;
use serde_json::Value;

// Use `10kB` as a threshold for 'auto'
// https://v8.dev/blog/cost-of-javascript-2019#json
pub const THRESHOLD_SIZE: usize = 10 * 1000;

/// /\.json(?:$|\?)(?!commonjs-(?:proxy|external))/
#[allow(clippy::case_sensitive_file_extension_comparisons)]
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

pub fn serialize_value(value: &Value) -> Result<String, serde_json::Error> {
  let value_as_string = serde_json::to_string(value)?;
  if value.is_object() && !value.is_null() && value_as_string.len() > THRESHOLD_SIZE {
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
    return "export default {{}};\n".to_string();
  }

  let mut named_export_code = String::new();
  let mut default_export_rows = Vec::with_capacity(data.len());

  for (key, value) in data {
    if rolldown_utils::ecmascript::is_validate_assignee_identifier_name(key) {
      writeln!(named_export_code, "export const {key} = {value};").unwrap();
      default_export_rows.push(Cow::Borrowed(key));
    } else {
      let key = serde_json::to_string(key).unwrap();
      default_export_rows.push(Cow::Owned(concat_string!(key, ": ", value.to_string())));
    }
  }

  let default_export_code =
    default_export_rows.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(",\n");

  concat_string!(named_export_code, "export default {\n", default_export_code, "\n};\n")
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
      "export const name = \"name\";\nexport default {\nname\n};\n",
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
      r#"export default {
"true": true,
"\\\"\n": 1234
};
"#,
      json_to_esm(&data, true)
    );
  }

  #[test]
  fn to_esm_named_exports_multiple_fields() {
    let data = serde_json::json!({"foo": "foo", "bar": "bar"});
    assert_eq!(
      "export const foo = \"foo\";\nexport const bar = \"bar\";\nexport default {\nfoo,\nbar\n};\n",
      json_to_esm(&data, true)
    );
  }
}
