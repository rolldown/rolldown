use std::borrow::Cow;
use std::fs::read_to_string;

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

    let start = args.id.rfind('/').unwrap_or(0);
    let Some(index) = args.id[start..].find(['?', '#']) else { return Ok(None) };

    let path = &args.id[..start + index];
    let code = read_to_string(path)?;

    ctx.add_watch_file(path);

    Ok(Some(HookLoadOutput { code, ..Default::default() }))
  }
}
