use std::borrow::Cow;

use rolldown_plugin::{HookUsage, Plugin};

#[derive(Debug)]
pub struct HtmlInlineProxyPlugin;

impl Plugin for HtmlInlineProxyPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:html-inline-proxy")
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::empty()
  }
}
