use std::borrow::Cow;

use rolldown_plugin::{HookLoadArgs, HookLoadReturn, Plugin, PluginContext};

#[derive(Debug)]
pub struct WasmFallbackPlugin {}

impl Plugin for WasmFallbackPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:wasm-fallback-plugin")
  }

  #[allow(clippy::case_sensitive_file_extension_comparisons)]
  async fn load(&self, _ctx: &PluginContext, args: &HookLoadArgs<'_>) -> HookLoadReturn {
    if args.id.ends_with(".wasm") {
      // TODO: Replace the link here after rolldown's document is ready
      Err(anyhow::anyhow!(
        "\"ESM integration proposal for Wasm\" is not supported currently.
        Use plugin-wasm or other community plugins to handle this.
        Alternatively, you can use `.wasm?init` or `.wasm?url`.
        See https://vitejs.dev/guide/features.html#webassembly for more details."
      ))
    } else {
      Ok(None)
    }
  }
}
