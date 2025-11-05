use std::borrow::Cow;

use rolldown_plugin::{HookResolveIdOutput, HookUsage, Plugin};

#[derive(Debug)]
pub struct HtmlInlineProxyPlugin;

impl Plugin for HtmlInlineProxyPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:html-inline-proxy")
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::ResolveId
  }

  async fn resolve_id(
    &self,
    _ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookResolveIdArgs<'_>,
  ) -> rolldown_plugin::HookResolveIdReturn {
    if rolldown_plugin_utils::find_special_query(args.specifier, b"html-proxy").is_some() {
      return Ok(Some(HookResolveIdOutput::from_id(args.specifier)));
    }
    Ok(None)
  }
}
