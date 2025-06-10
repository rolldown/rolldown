use std::borrow::Cow;

use arcstr::ArcStr;
use rolldown_common::{OutputFormat, side_effects::HookSideEffects};
use rolldown_plugin::{
  HookLoadArgs, HookLoadOutput, HookLoadReturn, HookResolveIdArgs, HookResolveIdOutput,
  HookResolveIdReturn, HookUsage, Plugin, PluginContext,
};

const MODULE_PRELOAD_POLYFILL: &str = "vite/modulepreload-polyfill";
const RESOLVED_MODULE_PRELOAD_POLYFILL_ID: &str = "\0vite/modulepreload-polyfill.js";

#[derive(Debug, Default)]
pub struct ModulePreloadPolyfillPlugin {
  pub is_server: bool,
}

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
      if self.is_server || !matches!(ctx.options().format, OutputFormat::Esm) {
        HookLoadOutput { code: ArcStr::new(), ..Default::default() }
      } else {
        HookLoadOutput {
          code: arcstr::literal!(include_str!("module-preload-polyfill.js")),
          side_effects: Some(HookSideEffects::True),
          ..Default::default()
        }
      }
    }))
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::ResolveId | HookUsage::Load
  }
}
