use std::borrow::Cow;

use rolldown::{BundlerOptions, InputItem};
use rolldown_common::EmittedChunk;
use rolldown_plugin::{HookUsage, Plugin, PluginContext};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};

#[derive(Debug)]
struct EmitEntryPlugin;

impl Plugin for EmitEntryPlugin {
  fn name(&self) -> Cow<'static, str> {
    "emit-entry-plugin".into()
  }

  async fn build_start(
    &self,
    ctx: &PluginContext,
    _args: &rolldown_plugin::HookBuildStartArgs<'_>,
  ) -> Result<(), anyhow::Error> {
    ctx.emit_chunk(EmittedChunk {
      id: "./z.js".to_string(),
      name: Some("entry".into()),
      ..Default::default()
    })?;
    Ok(())
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::BuildStart
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn dynamic_entry_order_emitted_root() {
  manual_integration_test!()
    .build(TestMeta::default())
    .run_with_plugins(
      BundlerOptions {
        input: Some(vec![InputItem { name: Some("main".into()), import: "./main.js".into() }]),
        ..Default::default()
      },
      vec![Plugin::new_shared(EmitEntryPlugin)],
    )
    .await;
}
