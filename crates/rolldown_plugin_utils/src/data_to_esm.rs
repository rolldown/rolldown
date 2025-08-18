use std::fmt::Write as _;

use rolldown_utils::concat_string;
use serde_json::Value;

use super::constants::THRESHOLD_SIZE;

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

pub fn data_to_esm(data: &Value, named_exports: bool) -> String {
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
    let data = serde_json::json!({"true": true, "\\\"\n": 1234});
    assert_eq!(
      "export default {\n  \"true\": true,\n  \"\\\\\\\"\\n\": 1234\n};",
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
}
