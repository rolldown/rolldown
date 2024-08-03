use rolldown_plugin::{
  HookLoadArgs, HookLoadOutput, HookLoadReturn, HookResolveIdArgs, HookResolveIdOutput,
  HookResolveIdReturn, Plugin, SharedPluginContext,
};
use std::borrow::Cow;

const MODULE_PRELOAD_POLYFILL: &str = "\0rolldown_module_preload_polyfill.js";

#[derive(Debug)]
pub struct ModulePreloadPolyfillPlugin {}

impl Plugin for ModulePreloadPolyfillPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("module_preload_polyfill")
  }

  async fn resolve_id(
    &self,
    _ctx: &SharedPluginContext,
    args: &HookResolveIdArgs<'_>,
  ) -> HookResolveIdReturn {
    if args.specifier == MODULE_PRELOAD_POLYFILL {
      Ok(Some(HookResolveIdOutput {
        id: MODULE_PRELOAD_POLYFILL.to_string(),
        ..Default::default()
      }))
    } else {
      Ok(None)
    }
  }

  async fn load(&self, _ctx: &SharedPluginContext, args: &HookLoadArgs<'_>) -> HookLoadReturn {
    if args.id == MODULE_PRELOAD_POLYFILL {
      return Ok(Some(HookLoadOutput {
        code: include_str!("module_preload_polyfill.js").to_string(),
        ..Default::default()
      }));
    }

    Ok(None)
  }
}
