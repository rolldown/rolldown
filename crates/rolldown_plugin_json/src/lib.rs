use rolldown_common::ModuleType;
use rolldown_plugin::{HookTransformOutput, Plugin};
use serde_json::Value;
use std::borrow::Cow;
use std::collections::HashSet;
use std::sync::OnceLock;

const RESERVED_WORDS: &str =
  "break case class catch const continue debugger default delete do else export extends finally for function if import in instanceof let new return super switch this throw try typeof var void while with yield enum await implements package protected static interface private public";
const BUILTINS: &str =
  "arguments Infinity NaN undefined null true false eval uneval isFinite isNaN parseFloat parseInt decodeURI decodeURIComponent encodeURI encodeURIComponent escape unescape Object Function Boolean Symbol Error EvalError InternalError RangeError ReferenceError SyntaxError TypeError URIError Number Math Date String RegExp Array Int8Array Uint8Array Uint8ClampedArray Int16Array Uint16Array Int32Array Uint32Array Float32Array Float64Array Map Set WeakMap WeakSet SIMD ArrayBuffer DataView JSON Promise Generator GeneratorFunction Reflect Proxy Intl";

#[derive(Debug, Default)]
pub struct JsonPlugin {
  pub stringify: bool,
  pub is_build: bool,
  pub named_exports: bool,
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
    let value = serde_json::from_str::<Value>(code)?;
    if self.stringify {
      let normalized_code = if self.is_build {
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

    let output = to_esm(value, self.named_exports);

    return Ok(Some(HookTransformOutput {
      code: Some(output),
      module_type: Some(ModuleType::Js),
      ..Default::default()
    }));
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

fn get_forbidden_identifiers() -> &'static HashSet<&'static str> {
  static FORBIDDEN_IDENTIFIERS: OnceLock<HashSet<&'static str>> = OnceLock::new();
  FORBIDDEN_IDENTIFIERS.get_or_init(|| {
    let mut set = std::collections::HashSet::from_iter(
      RESERVED_WORDS.split_whitespace().chain(BUILTINS.split_whitespace()),
    );
    set.insert("");
    set
  })
}

fn make_legal_identifier(ident: &str) -> String {
  let forbidden_identifiers = get_forbidden_identifiers();

  // convert hyphenated word to camel case
  let hyphen_re = regex::Regex::new(r"-(\w)").unwrap();
  let ident =
    hyphen_re.replace_all(ident, |capture: &regex::Captures<'_>| capture[1].to_ascii_uppercase());
  // convert invalid ch to underline
  let invalid_chars_re = regex::Regex::new(r"[^$_a-zA-Z0-9]").unwrap();
  let ident = invalid_chars_re.replace_all(&ident, "_");

  if ident.chars().nth(0).map_or(true, |ch| ch.is_ascii_digit())
    || forbidden_identifiers.contains(ident.as_ref())
  {
    return format!("_{ident}");
  }

  ident.to_string()
}

fn to_esm(data: Value, named_exports: bool) -> String {
  if !named_exports || !data.is_object() {
    return format!("export default {};\n", data);
  }

  let mut default_export_rows = vec![];
  let mut named_export_code = String::new();
  for (key, value) in data.as_object().unwrap() {
    if key == &make_legal_identifier(key) {
      default_export_rows.push(Cow::Borrowed(key));
      named_export_code += &format!("export const {key} = {value};\n");
    } else {
      default_export_rows.push(Cow::Owned(format!("\"{key}\": {value},\n",)));
    }
  }
  let default_export_code: String = default_export_rows.iter().map(|s| s.as_str()).collect();
  let default_export_code = format!("export default {{\n{default_export_code}\n}};\n");

  format!("{named_export_code}{default_export_code}")
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
    assert_eq!("export const name = \"name\";\nexport default {\nname\n};\n", to_esm(data, true));
  }

  #[test]
  fn to_esm_named_exports_literal() {
    let data = serde_json::json!(1);
    assert_eq!("export default 1;\n", to_esm(data, true));
  }

  #[test]
  fn to_esm_named_exports_forbidden_ident() {
    let data = serde_json::json!({"true": true});
    assert_eq!("export default {\n\"true\": true,\n\n};\n", to_esm(data, true));
  }
}
