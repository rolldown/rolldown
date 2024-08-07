use memchr::memchr2;
use rolldown_plugin::{HookLoadArgs, HookLoadOutput, HookLoadReturn, Plugin, SharedPluginContext};
use std::borrow::Cow;

#[derive(Debug)]
pub struct LoadFallbackPlugin {}

impl Plugin for LoadFallbackPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("load-fallback")
  }

  async fn load(&self, _ctx: &SharedPluginContext, args: &HookLoadArgs<'_>) -> HookLoadReturn {
    let normalized_id = if let Some(index) = memchr2(b'?', b'#', args.id.as_bytes()) {
      &args.id[0..index]
    } else {
      args.id
    };
    let code = std::fs::read_to_string(normalized_id)?;
    Ok(Some(HookLoadOutput { code, ..Default::default() }))
  }
}
