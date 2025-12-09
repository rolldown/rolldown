use std::borrow::Cow;

use rolldown_plugin::{HookUsage, Plugin};
use rolldown_utils::dashmap::FxDashSet;

#[derive(Debug)]
pub struct ViteWebWorkerPlugin {
  pub is_worker: bool,
  pub emitted_assets: FxDashSet<String>,
}

impl Plugin for ViteWebWorkerPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:vite-web-worker")
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::BuildStart
  }

  async fn build_start(
    &self,
    _ctx: &rolldown_plugin::PluginContext,
    _args: &rolldown_plugin::HookBuildStartArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    if !self.is_worker {
      self.emitted_assets.clear();
    }
    Ok(())
  }
}
