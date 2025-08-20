use std::borrow::Cow;

use rolldown_plugin::{HookUsage, Plugin};

#[derive(Debug, Default)]
pub struct RequireToImportPlugin;

impl Plugin for RequireToImportPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:require-to-import")
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::empty()
  }
}
