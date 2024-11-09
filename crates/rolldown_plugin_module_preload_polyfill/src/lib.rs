use rolldown_common::side_effects::HookSideEffects;
use rolldown_plugin::{
  HookLoadArgs, HookLoadOutput, HookLoadReturn, HookResolveIdArgs, HookResolveIdOutput,
  HookResolveIdReturn, Plugin, PluginContext,
};
use std::borrow::Cow;

const MODULE_PRELOAD_POLYFILL: &str = "vite/modulepreload-polyfill";

const RESOLVED_MODULE_PRELOAD_POLYFILL_ID: &str = "\0vite/modulepreload-polyfill.js";

const IS_MODERN_FLAG: &str = "__VITE_IS_MODERN__";

#[derive(Debug)]
pub struct ModulePreloadPolyfillPlugin {
  pub skip: bool,
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
    Ok((args.specifier == MODULE_PRELOAD_POLYFILL).then(|| HookResolveIdOutput {
      id: RESOLVED_MODULE_PRELOAD_POLYFILL_ID.to_string(),
      ..Default::default()
    }))
  }

  async fn load(&self, _ctx: &PluginContext, args: &HookLoadArgs<'_>) -> HookLoadReturn {
    if args.id == RESOLVED_MODULE_PRELOAD_POLYFILL_ID {
      if self.skip {
        return Ok(Some(HookLoadOutput { code: String::new(), ..Default::default() }));
      }

      Ok(Some(HookLoadOutput {
        code: format!("{IS_MODERN_FLAG}&&{}", include_str!("module_preload_polyfill.js")),
        side_effects: Some(HookSideEffects::True),
        ..Default::default()
      }))
    } else {
      Ok(None)
    }
  }
}
