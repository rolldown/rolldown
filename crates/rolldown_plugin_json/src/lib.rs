use std::borrow::Cow;
use std::fmt::Write as _;

use rolldown_common::ModuleType;
use rolldown_plugin::{HookTransformOutput, Plugin};
use rolldown_sourcemap::SourceMap;
use serde_json::Value;

#[derive(Debug, Default)]
pub struct JsonPlugin {
  pub stringify: JsonPluginStringify,
  pub is_build: bool,
  pub named_exports: bool,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JsonPluginStringify {
  False,
  True,
  #[default]
  Auto,
}

impl Plugin for JsonPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:json")
  }

  async fn transform(
    &self,
    _ctx: rolldown_plugin::SharedTransformPluginContext,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    // Not sure we should use `module type to filter, but for now prefer to follow vite behavior`
    if !is_json_ext(args.id) || is_special_query(args.id) {
      return Ok(None);
    }
    let code = strip_bom(args.code);

    if self.stringify != JsonPluginStringify::False {
      if self.named_exports && code.trim_start().starts_with('{') {
        let parsed = serde_json::from_str::<Value>(code)?;
        let parsed =
          parsed.as_object().expect("should be object because the value starts with `{`");

        let mut code = String::new();
        let mut default_object_code = "{\n".to_owned();
        for (key, value) in parsed {
          if rolldown_utils::ecmascript::is_validate_assignee_identifier_name(key) {
            writeln!(code, "export const {key} = {};", &serialize_value(value)?).unwrap();
            writeln!(default_object_code, "  {key},").unwrap();
          } else {
            let key = serde_json::to_string(key).unwrap();
            writeln!(default_object_code, "  {key}: {},", &serialize_value(value)?).unwrap();
          }
        }
        default_object_code += "}";

        writeln!(code, "export default {default_object_code};").unwrap();

        return Ok(Some(HookTransformOutput {
          code: Some(code),
          map: Some(SourceMap::default()),
          module_type: Some(ModuleType::Js),
          ..Default::default()
        }));
      }

      if self.stringify == JsonPluginStringify::True ||
      // use 10kB as a threshold for 'auto'
      // https://v8.dev/blog/cost-of-javascript-2019#:~:text=A%20good%20rule%20of%20thumb%20is%20to%20apply%20this%20technique%20for%20objects%20of%2010%20kB%20or%20larger
      code.len() > 10 * 1000
      {
        let normalized_code = if self.is_build {
          // TODO: perf: find better way than https://github.com/rolldown/vite/blob/3bf86e3f715c952a032b476b60c8c869e9c50f3f/packages/vite/src/node/plugins/json.ts#L55-L57
          let value = serde_json::from_str::<Value>(code)?;
          Cow::Owned(serde_json::to_string(&value)?)
        } else {
          Cow::Borrowed(code)
        };
        let normalized_code_string = serde_json::to_string(&normalized_code)?;
        return Ok(Some(HookTransformOutput {
          code: Some(format!("export default /*#__PURE__*/ JSON.parse({normalized_code_string})")),
          map: Some(SourceMap::default()),
          module_type: Some(ModuleType::Js),
          ..Default::default()
        }));
      }
    }

    let value = serde_json::from_str::<Value>(code)?;
    let output = to_esm(&value, self.named_exports);
    Ok(Some(HookTransformOutput {
      code: Some(output),
      map: Some(SourceMap::default()),
      module_type: Some(ModuleType::Js),
      ..Default::default()
    }))
  }
}

// cSpell:disable
fn strip_bom(code: &str) -> &str {
  if let Some(stripped) = code.strip_prefix("\u{FEFF}") { stripped } else { code }
}

/// /\.json(?:$|\?)(?!commonjs-(?:proxy|external))/
#[allow(clippy::case_sensitive_file_extension_comparisons)]
fn is_json_ext(ext: &str) -> bool {
  if ext.ends_with(".json") {
    return true;
  }
  let Some(i) = memchr::memmem::rfind(ext.as_bytes(), ".json?".as_bytes()) else {
    return false;
  };
  let postfix = &ext[i + 6..];
  postfix != "commonjs-proxy" && postfix != "commonjs-external"
}

/// SPECIAL_QUERY_RE = /[?&](?:worker|sharedworker|raw|url)\b/
fn is_special_query(ext: &str) -> bool {
  for i in memchr::memrchr2_iter(b'?', b'&', ext.as_bytes()) {
    let Some(after) = ext.get(i + 1..) else { continue };
    let boundary = if after.starts_with("worker") {
      6usize
    } else if after.starts_with("sharedworker") {
      12usize
    } else if after.starts_with("raw") || after.starts_with("url") {
      3usize
    } else {
      continue;
    };
    // test if match `\b`
    match after.get(boundary..=boundary).and_then(|c| c.bytes().next()) {
      Some(ch) if !matches!(ch, b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z' | b'_') => {
        return true;
      }
      None => return true,
      _ => {}
    }
  }
  false
}

fn serialize_value(value: &Value) -> Result<String, serde_json::Error> {
  let value_as_string = serde_json::to_string(value)?;
  if value.is_object() && !value.is_null() && value_as_string.len() > 10 * 1000 {
    Ok(format!("/*#__PURE__*/ JSON.parse({})", serde_json::to_string(&value_as_string)?))
  } else {
    Ok(value_as_string)
  }
}

fn to_esm(data: &Value, named_exports: bool) -> String {
  if !named_exports || !data.is_object() {
    return format!("export default {data};\n");
  }

  let mut default_export_rows = vec![];
  let mut named_export_code = String::new();
  for (key, value) in data.as_object().unwrap() {
    if rolldown_utils::ecmascript::is_validate_assignee_identifier_name(key) {
      default_export_rows.push(Cow::Borrowed(key));
      writeln!(named_export_code, "export const {key} = {value};").unwrap();
    } else {
      let key = serde_json::to_string(key).unwrap();
      default_export_rows.push(Cow::Owned(format!("{key}: {value}",)));
    }
  }

  let default_export_code =
    default_export_rows.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(",\n");

  format!("{named_export_code}export default {{\n{default_export_code}\n}};\n")
}

#[cfg(test)]
mod test {
  use crate::{is_json_ext, is_special_query, to_esm};

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
    assert_eq!("export const name = \"name\";\nexport default {\nname\n};\n", to_esm(&data, true));
  }

  #[test]
  fn to_esm_named_exports_literal() {
    let data = serde_json::json!(1);
    assert_eq!("export default 1;\n", to_esm(&data, true));
  }

  #[test]
  fn to_esm_named_exports_forbidden_ident() {
    let data = serde_json::json!({"true": true, "\\\"\n": 1234});
    assert_eq!(
      r#"
export default {
"true": true,
"\\\"\n": 1234
};
"#
      .trim_start(),
      to_esm(&data, true)
    );
  }

  #[test]
  fn to_esm_named_exports_multiple_fields() {
    let data = serde_json::json!({"foo": "foo", "bar": "bar"});
    assert_eq!(
      "export const foo = \"foo\";\nexport const bar = \"bar\";\nexport default {\nfoo,\nbar\n};\n",
      to_esm(&data, true)
    );
  }
}
