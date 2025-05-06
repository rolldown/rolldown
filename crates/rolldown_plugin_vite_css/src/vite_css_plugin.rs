use rolldown_plugin::Plugin;

#[derive(Debug)]
pub struct ViteCssPlugin;

impl Plugin for ViteCssPlugin {
  fn name(&self) -> std::borrow::Cow<'static, str> {
    std::borrow::Cow::Borrowed("builtin:vite-css")
  }

  fn register_hook_usage(&self) -> rolldown_plugin::HookUsage {
    rolldown_plugin::HookUsage::empty()
  }
}
