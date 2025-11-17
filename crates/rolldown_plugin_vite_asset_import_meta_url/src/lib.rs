use std::borrow::Cow;

use rolldown_plugin::{HookUsage, Plugin};

#[derive(Debug)]
pub struct ViteAssetImportMetaUrlPlugin;

impl Plugin for ViteAssetImportMetaUrlPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:vite-asset-import-meta-url")
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::empty()
  }
}
