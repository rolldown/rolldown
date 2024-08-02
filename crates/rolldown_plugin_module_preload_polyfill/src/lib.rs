use rolldown_common::side_effects::HookSideEffects;
use rolldown_plugin::{
  HookLoadArgs, HookLoadOutput, HookLoadReturn, HookResolveIdArgs, HookResolveIdOutput,
  HookResolveIdReturn, Plugin, SharedPluginContext,
};
use std::borrow::Cow;

const MODULE_PRELOAD_POLYFILL: &str = "\0rolldown_module_preload_polyfill.js";

const IS_MODERN_FLAG: &'static str = "__VITE_IS_MODERN__";

#[derive(Debug)]
pub struct ModulePreloadPolyfillPlugin {
  pub skip: bool,
}

impl Plugin for ModulePreloadPolyfillPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("module_preload_polyfill")
  }

  async fn resolve_id(
    &self,
    _ctx: &SharedPluginContext,
    args: &HookResolveIdArgs<'_>,
  ) -> HookResolveIdReturn {
    Ok((args.specifier == MODULE_PRELOAD_POLYFILL).then(|| HookResolveIdOutput {
      id: MODULE_PRELOAD_POLYFILL.to_string(),
      ..Default::default()
    }))
  }

  async fn load(&self, _ctx: &SharedPluginContext, args: &HookLoadArgs<'_>) -> HookLoadReturn {
    if self.skip {
      return Ok(None);
    }
    Ok((args.id == MODULE_PRELOAD_POLYFILL).then(|| HookLoadOutput {
      code: format!("{IS_MODERN_FLAG}&&{}", include_str!("module_preload_polyfill.js")),
      side_effects: Some(HookSideEffects::True),
      ..Default::default()
    }))
  }
}
