use std::borrow::Cow;

use rolldown_plugin::{HookUsage, Plugin};

#[derive(Debug)]
pub struct AssetImportMetaUrlPlugin;

impl Plugin for AssetImportMetaUrlPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:asset-import-meta-url")
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::empty()
  }
}
