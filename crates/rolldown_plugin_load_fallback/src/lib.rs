use std::borrow::Cow;

use rolldown_plugin::{HookLoadArgs, HookLoadOutput, HookLoadReturn, Plugin, PluginContext};
use rolldown_utils::{clean_url::clean_url, dataurl::parse_data_url};

#[derive(Debug)]
pub struct LoadFallbackPlugin {}

impl Plugin for LoadFallbackPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:load-fallback")
  }

  async fn load(&self, _ctx: &PluginContext, args: &HookLoadArgs<'_>) -> HookLoadReturn {
    if parse_data_url(args.id).is_some() {
      return Ok(None);
    }
    let normalized_id = clean_url(args.id);
    let code =
      std::fs::read_to_string(normalized_id).or_else(|_| std::fs::read_to_string(args.id))?;
    Ok(Some(HookLoadOutput { code, ..Default::default() }))
  }
}
