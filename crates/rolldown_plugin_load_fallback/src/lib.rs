use rolldown_plugin::{HookLoadArgs, HookLoadOutput, HookLoadReturn, Plugin, PluginContext};
use rolldown_utils::path_ext::clean_url;
use std::borrow::Cow;

#[derive(Debug)]
pub struct LoadFallbackPlugin {}

impl Plugin for LoadFallbackPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:load-fallback")
  }

  async fn load(&self, _ctx: &PluginContext, args: &HookLoadArgs<'_>) -> HookLoadReturn {
    let normalized_id = clean_url(args.id);
    let code = std::fs::read_to_string(normalized_id)?;
    Ok(Some(HookLoadOutput { code, ..Default::default() }))
  }
}
