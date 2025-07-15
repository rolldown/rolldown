use std::borrow::Cow;

use rolldown_plugin::{HookUsage, Plugin};

#[derive(Debug, Default)]
pub struct ChunkImportMapPlugin;

impl Plugin for ChunkImportMapPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:chunk-import-map")
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::empty()
  }
}
