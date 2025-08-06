use std::borrow::Cow;

use rolldown_plugin::{HookLoadArgs, HookLoadReturn, HookUsage, Plugin, PluginContext};

#[derive(Debug)]
pub struct WasmFallbackPlugin;

impl Plugin for WasmFallbackPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:wasm-fallback")
  }

  async fn load(&self, _ctx: &PluginContext, args: &HookLoadArgs<'_>) -> HookLoadReturn {
    if args.id.ends_with(".wasm") {
      // TODO: Replace the link here after rolldown's document is ready
      Err(anyhow::anyhow!(
        "\"ESM integration proposal for Wasm\" is not supported currently.
        Use plugin-wasm or other community plugins to handle this.
        Alternatively, you can use `.wasm?init` or `.wasm?url`.
        See https://vitejs.dev/guide/features.html#webassembly for more details."
      ))?;
    }
    Ok(None)
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Load
  }
}
