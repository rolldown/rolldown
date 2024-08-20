use rolldown_common::ModuleType;
use rolldown_plugin::{HookTransformOutput, Plugin};
use serde_json::Value;
use std::borrow::Cow;

#[derive(Debug, Default)]
pub struct JsonPlugin {
  pub stringify: bool,
  pub is_build: bool, // TODO: support namedExports in rolldown json https://github.com/rolldown/vite/blob/3bf86e3f715c952a032b476b60c8c869e9c50f3f/packages/vite/src/node/plugins/json.ts#L69-L69
}

impl Plugin for JsonPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:json")
  }

  async fn transform(
    &self,
    _ctx: &rolldown_plugin::TransformPluginContext<'_>,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    // Not sure we should use `module type to filter, but for now prefer to follow vite behavior`
    if !is_json_ext(args.id) || is_special_query(args.id) {
      return Ok(None);
    }
    let code = strip_bom(args.code);
    if self.stringify {
      let normalized_code = if self.is_build {
        let value = serde_json::from_str::<Value>(code)?;
        let str = serde_json::to_string(&value)?;
        // TODO: perf: find better way than https://github.com/rolldown/vite/blob/3bf86e3f715c952a032b476b60c8c869e9c50f3f/packages/vite/src/node/plugins/json.ts#L55-L57
        let str = serde_json::to_string(&str)?;
        format!("export default /*#__PURE__*/ JSON.parse({str})")
      } else {
        format!("export default /*#__PURE__*/ JSON.parse({})", serde_json::to_string(code)?)
      };
      return Ok(Some(HookTransformOutput {
        code: Some(normalized_code),
        module_type: Some(ModuleType::Js),
        ..Default::default()
      }));
    }
    // let default json handler to transform json
    Ok(None)
  }
}

// cSpell:disable
fn strip_bom(code: &str) -> &str {
  if let Some(stripped) = code.strip_prefix("\u{FEFF}") {
    stripped
  } else {
    code
  }
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
      _ => continue,
    }
  }
  false
}

#[cfg(test)]
mod test {
  use crate::{is_json_ext, is_special_query};

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
}
