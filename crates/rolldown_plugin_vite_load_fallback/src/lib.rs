use std::borrow::Cow;

use rolldown_plugin::{
  HookLoadArgs, HookLoadOutput, HookLoadReturn, HookUsage, Plugin, SharedLoadPluginContext,
};
use rolldown_utils::dataurl::is_data_url;

#[derive(Debug)]
pub struct ViteLoadFallbackPlugin;

impl Plugin for ViteLoadFallbackPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:vite-load-fallback")
  }

  async fn load(&self, ctx: SharedLoadPluginContext, args: &HookLoadArgs<'_>) -> HookLoadReturn {
    if is_data_url(args.id) {
      return Ok(None);
    }

    let Some(index) = memchr::memchr2(b'?', b'#', args.id.as_bytes()) else {
      return Ok(None);
    };

    let path = &args.id[..index];
    let Ok(code) = std::fs::read_to_string(path) else { return Ok(None) };

    ctx.add_watch_file(path);

    Ok(Some(HookLoadOutput { code: code.into(), ..Default::default() }))
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Load
  }
}
