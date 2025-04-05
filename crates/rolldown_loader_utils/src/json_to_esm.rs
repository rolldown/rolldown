use std::fmt::Write as _;

use rolldown_utils::ecmascript::is_validate_assignee_identifier_name;
use serde_json::{Value, to_string_pretty};

// TODO: handling https://github.com/tc39/proposal-json-superset
pub fn json_to_esm(json: &str) -> serde_json::Result<String> {
  // TODO: use zero-copy deserialization
  let json_value = serde_json::from_str(json.trim_start_matches("\u{FEFF}"))?;

  match json_value {
    Value::Object(map) => {
      let mut source = String::new();
      let mut exported_items_for_default_export = Vec::with_capacity(map.len());
      for (idx, (key, value)) in map.iter().enumerate() {
        if is_validate_assignee_identifier_name(key) {
          writeln!(source, "export const {key} = {};", to_string_pretty(value)?).unwrap();
          exported_items_for_default_export.push(key.to_string());
        } else {
          let valid_id = format!("key_{}", itoa::Buffer::new().format(idx));

          writeln!(source, "const {valid_id} = {};", to_string_pretty(value)?).unwrap();
          writeln!(source, "export {{ {valid_id} as '{key}' }};").unwrap();

          exported_items_for_default_export.push(format!("'{key}': {valid_id}"));
        }
      }
      write!(source, "export default {{ {} }};", exported_items_for_default_export.join(", "))
        .unwrap();

      Ok(source)
    }
    _ => {
      let json_str = serde_json::to_string(&json_value)?;
      Ok(format!("export default {json_str}"))
    }
  }
}
