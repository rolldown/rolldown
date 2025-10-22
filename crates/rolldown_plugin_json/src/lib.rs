mod utils;

use std::borrow::Cow;

use rolldown_common::ModuleType;
use rolldown_plugin::{HookTransformOutput, HookUsage, Plugin};
use rolldown_plugin_utils::{constants, data_to_esm, is_special_query};
use rolldown_sourcemap::SourceMap;
use rolldown_utils::concat_string;
use serde_json::Value;

#[derive(Debug, Default)]
pub struct JsonPlugin {
  pub minify: bool,
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
    if *args.module_type != ModuleType::Json
      || !utils::is_json_ext(args.id)
      || is_special_query(args.id)
    {
      return Ok(None);
    }

    let code = rolldown_plugin_utils::strip_bom(args.code);

    let is_name_exports = self.named_exports && code.trim_start().starts_with('{');
    let is_stringify = self.stringify != JsonPluginStringify::False
      && (self.stringify == JsonPluginStringify::True || code.len() > constants::THRESHOLD_SIZE);
    if !is_name_exports && is_stringify {
      let json = if self.minify {
        // TODO(perf): find better way than https://github.com/rolldown/vite/blob/3bf86e3f/packages/vite/src/node/plugins/json.ts#L55-L57
        let value = serde_json::from_str::<Value>(code)
          .map_err(|e| anyhow::anyhow!("Failed to parse JSON: {e}"))?;
        Cow::Owned(
          serde_json::to_string(&value)
            .map_err(|e| anyhow::anyhow!("Failed to stringify JSON: {e}"))?,
        )
      } else {
        Cow::Borrowed(code)
      };

      return Ok(Some(HookTransformOutput {
        code: Some(concat_string!(
          "export default /*#__PURE__*/ JSON.parse(",
          serde_json::to_string(&json)
            .map_err(|e| anyhow::anyhow!("Failed to stringify JSON: {e}"))?,
          ")"
        )),
        map: Some(SourceMap::default()),
        module_type: Some(ModuleType::Js),
        ..Default::default()
      }));
    }

    let value =
      serde_json::from_str(code).map_err(|e| anyhow::anyhow!("Failed to parse JSON: {e}"))?;
    Ok(Some(HookTransformOutput {
      code: Some(data_to_esm(&value, self.named_exports)),
      map: Some(SourceMap::default()),
      module_type: Some(ModuleType::Js),
      ..Default::default()
    }))
  }

  fn register_hook_usage(&self) -> rolldown_plugin::HookUsage {
    HookUsage::Transform
  }
}
