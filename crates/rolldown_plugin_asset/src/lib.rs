use std::borrow::Cow;

use rolldown_plugin::{HookUsage, Plugin};

#[derive(Debug, Default)]
pub struct AssetPlugin;

impl Plugin for AssetPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:asset")
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::empty()
  }
}
