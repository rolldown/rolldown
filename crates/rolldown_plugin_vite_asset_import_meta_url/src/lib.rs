mod utils;

use std::borrow::Cow;

use rolldown_plugin::{HookUsage, Plugin};

#[derive(Debug)]
pub struct ViteAssetImportMetaUrlPlugin {
  pub client_entry: String,
}

impl Plugin for ViteAssetImportMetaUrlPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:vite-asset-import-meta-url")
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Transform
  }

  async fn transform(
    &self,
    _ctx: rolldown_plugin::SharedTransformPluginContext,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    if args.id == utils::PRELOAD_HELPER_ID
      || args.id == self.client_entry
      || !utils::contains_asset_import_meta_url(args.code)
    {
      return Ok(None);
    }

    Ok(None)
  }
}
