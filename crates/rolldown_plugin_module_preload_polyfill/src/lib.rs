use std::borrow::Cow;

use rolldown_common::{OutputFormat, side_effects::HookSideEffects};
use rolldown_plugin::{
  HookLoadArgs, HookLoadOutput, HookLoadReturn, HookResolveIdArgs, HookResolveIdOutput,
  HookResolveIdReturn, Plugin, PluginContext,
};

const MODULE_PRELOAD_POLYFILL: &str = "vite/modulepreload-polyfill";
const RESOLVED_MODULE_PRELOAD_POLYFILL_ID: &str = "\0vite/modulepreload-polyfill.js";

#[derive(Debug)]
pub struct ModulePreloadPolyfillPlugin;

impl Plugin for ModulePreloadPolyfillPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:module-preload-polyfill")
  }

  async fn resolve_id(
    &self,
    _ctx: &PluginContext,
    args: &HookResolveIdArgs<'_>,
  ) -> HookResolveIdReturn {
    Ok((args.specifier == MODULE_PRELOAD_POLYFILL).then_some(HookResolveIdOutput {
      id: arcstr::literal!(RESOLVED_MODULE_PRELOAD_POLYFILL_ID),
      ..Default::default()
    }))
  }

  async fn load(&self, ctx: &PluginContext, args: &HookLoadArgs<'_>) -> HookLoadReturn {
    Ok((args.id == RESOLVED_MODULE_PRELOAD_POLYFILL_ID).then(|| {
      if matches!(ctx.options().format, OutputFormat::Esm) {
        HookLoadOutput {
          code: include_str!("module-preload-polyfill.js").to_string(),
          side_effects: Some(HookSideEffects::True),
          ..Default::default()
        }
      } else {
        HookLoadOutput { code: String::new(), ..Default::default() }
      }
    }))
  }
}
