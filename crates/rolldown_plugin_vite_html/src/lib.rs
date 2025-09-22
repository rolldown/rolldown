use std::borrow::Cow;

use rolldown_plugin::{HookUsage, Plugin};

#[derive(Debug, Default)]
pub struct ViteHtmlPlugin;

impl Plugin for ViteHtmlPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:vite-html")
  }

  fn register_hook_usage(&self) -> rolldown_plugin::HookUsage {
    HookUsage::Transform | HookUsage::GenerateBundle
  }

  async fn transform(
    &self,
    _ctx: rolldown_plugin::SharedTransformPluginContext,
    _args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    todo!()
  }

  async fn generate_bundle(
    &self,
    _ctx: &rolldown_plugin::PluginContext,
    _args: &mut rolldown_plugin::HookGenerateBundleArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    todo!()
  }
}
