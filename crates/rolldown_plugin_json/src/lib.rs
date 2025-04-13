mod utils;

use std::borrow::Cow;
use std::fmt::Write as _;

use rolldown_common::ModuleType;
use rolldown_plugin::{HookTransformOutput, Plugin};
use rolldown_sourcemap::SourceMap;
use rolldown_utils::concat_string;
use serde_json::Value;

#[derive(Debug, Default)]
pub struct JsonPlugin {
  pub is_build: bool,
  pub named_exports: bool,
  pub stringify: JsonPluginStringify,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JsonPluginStringify {
  #[default]
  Auto,
  True,
  False,
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
    // Not sure we should use `module_type` to filter, but for now prefer to follow vite behavior
    if !utils::is_json_ext(args.id) || utils::is_special_query(args.id) {
      return Ok(None);
    }

    let code = utils::strip_bom(args.code);

    if self.stringify != JsonPluginStringify::False {
      if self.named_exports && code.trim_start().starts_with('{') {
        let parsed = serde_json::from_str::<Value>(code)?;
        let parsed = parsed.as_object().expect("should be object because it starts with `{`");

        let mut code = String::new();
        let mut default_object_code = "{\n".to_owned();
        for (key, value) in parsed {
          if rolldown_utils::ecmascript::is_validate_assignee_identifier_name(key) {
            writeln!(code, "export const {key} = {};", &utils::serialize_value(value)?).unwrap();
            writeln!(default_object_code, "  {key},").unwrap();
          } else {
            let key = serde_json::to_string(key).unwrap();
            writeln!(default_object_code, "  {key}: {},", &utils::serialize_value(value)?).unwrap();
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

      if self.stringify == JsonPluginStringify::True || code.len() > utils::THRESHOLD_SIZE {
        let json = if self.is_build {
          // TODO(perf): find better way than https://github.com/rolldown/vite/blob/3bf86e3f/packages/vite/src/node/plugins/json.ts#L55-L57
          let value = serde_json::from_str::<Value>(code)?;
          Cow::Owned(serde_json::to_string(&value)?)
        } else {
          Cow::Borrowed(code)
        };

        return Ok(Some(HookTransformOutput {
          code: Some(concat_string!(
            "export default /*#__PURE__*/ JSON.parse(",
            serde_json::to_string(&json)?,
            ")"
          )),
          map: Some(SourceMap::default()),
          module_type: Some(ModuleType::Js),
          ..Default::default()
        }));
      }
    }

    let value = serde_json::from_str(code)?;
    let code = utils::json_to_esm(&value, self.named_exports);
    Ok(Some(HookTransformOutput {
      code: Some(code),
      map: Some(SourceMap::default()),
      module_type: Some(ModuleType::Js),
      ..Default::default()
    }))
  }
}
