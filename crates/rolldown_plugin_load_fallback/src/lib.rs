use std::borrow::Cow;

use rolldown_plugin::{
  HookLoadArgs, HookLoadOutput, HookLoadReturn, HookUsage, Plugin, PluginContext,
};

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

    let path = memchr::memchr2(b'?', b'#', args.id.as_bytes()).map_or(args.id, |i| &args.id[..i]);
    let code = std::fs::read_to_string(path).or_else(|err| {
      if path.len() == args.id.len() { Err(err) } else { std::fs::read_to_string(args.id) }
    })?;

    ctx.add_watch_file(path);

    Ok(Some(HookLoadOutput { code, ..Default::default() }))
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Load
  }
}
