use anyhow::Ok;
use rolldown_utils::ecma_script::is_validate_identifier_name;
use serde_json::Value;
// TODO: handling https://github.com/tc39/proposal-json-superset

pub fn json_to_esm(json: &str) -> anyhow::Result<String> {
  // TODO: use zero-copy deserialization
  let json_value: Value = serde_json::from_str(json)?;

  match json_value {
    Value::Object(map) => {
      let mut source = String::new();
      let mut exported_items_for_default_export = Vec::with_capacity(map.len());
      for (idx, (key, value)) in map.iter().enumerate() {
        if is_validate_identifier_name(key) {
          source
            .push_str(&format!("export const {key} = {};\n", serde_json::to_string_pretty(value)?));
          exported_items_for_default_export.push(key.to_string());
        } else {
          let valid_id = format!("key_{idx}");
          source.push_str(&format!(
            "const {} = {};\n",
            valid_id,
            serde_json::to_string_pretty(value)?
          ));
          source.push_str(&format!("export {{ {valid_id} as '{key}' }};\n"));
          exported_items_for_default_export.push(format!("'{key}': {valid_id}"));
        };
      }
      source.push_str(&format!(
        "export default {{ {} }};",
        exported_items_for_default_export.join(", ")
      ));
      Ok(source)
    }
    _ => {
      let json_str = serde_json::to_string(&json_value)?;
      Ok(format!("export default {json_str}"))
    }
  }
}
