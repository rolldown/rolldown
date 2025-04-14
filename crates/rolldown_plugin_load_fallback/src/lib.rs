use std::borrow::Cow;

use rolldown_plugin::{HookLoadArgs, HookLoadOutput, HookLoadReturn, Plugin, PluginContext};

#[derive(Debug)]
pub struct LoadFallbackPlugin;

impl Plugin for LoadFallbackPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:load-fallback")
  }

  async fn load(&self, ctx: &PluginContext, args: &HookLoadArgs<'_>) -> HookLoadReturn {
    if args.id.trim_start().starts_with("data:") {
      return Ok(None);
    }

    let Some(index) = memchr::memchr2(b'?', b'#', args.id.as_bytes()) else {
      return Ok(None);
    };

    let path = &args.id[..index];
    let Ok(code) = std::fs::read_to_string(path) else { return Ok(None) };

    ctx.add_watch_file(path);

    Ok(Some(HookLoadOutput { code, ..Default::default() }))
  }
}
