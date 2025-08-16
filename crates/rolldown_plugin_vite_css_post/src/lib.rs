use rolldown_plugin::{HookUsage, Plugin};

#[derive(Debug)]
pub struct ViteCssPostPlugin;

impl Plugin for ViteCssPostPlugin {
  fn name(&self) -> std::borrow::Cow<'static, str> {
    std::borrow::Cow::Borrowed("builtin:vite-css-post")
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::empty()
  }
}
