use regex::Regex;
use rolldown_plugin::{HookLoadArgs, HookLoadOutput, HookLoadReturn, Plugin, PluginContext};
use rolldown_utils::clean_url::clean_url;
use std::{borrow::Cow, sync::LazyLock};

static DATA_URL_RE: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new("^data:([^/]+\\/[^;]+)(;charset=[^;]+)?(;base64)?,([\\s\\S]*)$").unwrap()
});

#[derive(Debug)]
pub struct LoadFallbackPlugin {}

impl Plugin for LoadFallbackPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:load-fallback")
  }

  async fn load(&self, _ctx: &PluginContext, args: &HookLoadArgs<'_>) -> HookLoadReturn {
    if DATA_URL_RE.is_match(args.id) {
      return Ok(None);
    }
    let normalized_id = clean_url(args.id);
    let code =
      std::fs::read_to_string(normalized_id).or_else(|_| std::fs::read_to_string(args.id))?;
    Ok(Some(HookLoadOutput { code, ..Default::default() }))
  }
}
